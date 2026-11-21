import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import * as os from 'node:os';
import {randomUUID} from 'node:crypto';
import semver from 'semver';
import type {DependencyPlan, DependencyRuntime, Proposal, TestProfile} from '../common/forge-protocol.js';
import {command, requireLocalDocker, sha, snapshot} from './verification.js';

type Inputs={files:Record<string,string>;packages:number;digest:string};
const pinned=(profile:TestProfile,base:string):boolean=>new RegExp('^'+profile+'@sha256:[a-f0-9]{64}$').test(base);
/** Validate the complete installation plan before any network request or Docker build. */
export async function dependencyInputs(dir:string,profile:TestProfile,base:string):Promise<Inputs> {
 if(!['node','python'].includes(profile)||!pinned(profile,base))throw new Error('Choose an official runtime pinned by digest');
 const read=async(name:string):Promise<string>=>{const file=path.join(dir,name),st=await fs.lstat(file);if(!st.isFile()||st.isSymbolicLink()||st.size>2*1024*1024)throw new Error('Unsupported dependency manifest: '+name);return fs.readFile(file,'utf8');};
 const files:Record<string,string>={};let packages=0;
 if(profile==='node') {
  const manifest=JSON.parse(await read('package.json')),raw=await read('package-lock.json'),lock=JSON.parse(raw);
  if(manifest.workspaces || ![2,3].includes(lock.lockfileVersion)||!lock.packages||typeof lock.packages!=='object')throw new Error('Use a single-package npm project with package-lock v2 or v3; workspaces are not supported yet');
  const clean:Record<string,unknown>={name:'forge-verification-project',version:manifest.version||'0.0.0',private:true};
  for(const kind of ['dependencies','devDependencies','optionalDependencies'])if(manifest[kind]) {
   if(typeof manifest[kind]!=='object'||Array.isArray(manifest[kind]))throw new Error('Invalid dependency table');
   for(const [name,spec] of Object.entries(manifest[kind]))if(!/^(@[a-z0-9._-]+\/)?[a-z0-9._-]+$/i.test(name)||typeof spec!=='string'||!semver.validRange(spec))throw new Error('Only npm registry packages with semver ranges are supported');
   clean[kind]=manifest[kind];
  }
  for(const [location,entry] of Object.entries(lock.packages) as [string,Record<string,unknown>][]) {
   if(location==='')continue;
   if(!location.startsWith('node_modules/')||location.split('/').some(s=>s==='..'||s==='.')||location.includes('\\')||entry.link)throw new Error('Local, linked or workspace dependencies are not supported');
   if(typeof entry.resolved!=='string'||typeof entry.integrity!=='string'||!/^sha(256|384|512)-[a-zA-Z0-9+/=]+$/.test(entry.integrity))throw new Error('Every dependency needs a public URL and a strong integrity hash');
   const u=new URL(entry.resolved);
   if(u.protocol!=='https:'||u.hostname!=='registry.npmjs.org'||u.port||u.username||u.password||u.search||u.hash)throw new Error('Only public registry.npmjs.org dependencies are allowed; credentials and mirrors are excluded');
   if(++packages>512)throw new Error('Dependency profile is limited to 512 locked packages');
  }
  // npm ci checks manifest/lock consistency. Lifecycle scripts and project npmrc are never copied.
  files['package.json']=JSON.stringify(clean);files['package-lock.json']=raw;
 } else {
  const raw=await read('requirements.txt'),lines=raw.replace(/\\\r?\n\s*/g,' ').split(/\r?\n/),names=new Set<string>();const normalized:string[]=[];
  for(const rawLine of lines) {
   const line=rawLine.trim();if(!line||line.startsWith('#'))continue;
   const m=/^([a-zA-Z0-9][a-zA-Z0-9._-]*)==([a-zA-Z0-9][a-zA-Z0-9.!+_-]*)(\s+--hash=sha256:[a-fA-F0-9]{64})+$/.exec(line);
   if(!m)throw new Error('Python requires exact name==version and SHA-256 wheel hashes on every requirement; URLs, options, markers and source builds are excluded');
   const name=m[1].toLowerCase().replace(/[-_.]+/g,'-');if(names.has(name))throw new Error('Duplicate Python requirement');names.add(name);normalized.push(line);
   if(++packages>512)throw new Error('Dependency profile is limited to 512 locked packages');
  }
  files['requirements.txt']=normalized.join('\n')+'\n';
 }
 if(!packages)throw new Error('No locked dependencies found; use the base runtime instead');
 return {files,packages,digest:sha(JSON.stringify({schema:1,profile,base,files}))};
}
export function dependencyDockerfile(profile:TestProfile,base:string,digest:string):string {
 if(!pinned(profile,base)||!/^[a-f0-9]{64}$/.test(digest))throw new Error('Invalid runtime plan');
 const install=profile==='node'?
  'WORKDIR /opt/forge/deps\nCOPY package.json package-lock.json ./\nRUN npm ci --ignore-scripts --no-audit --no-fund --include=dev --no-progress --registry=https://registry.npmjs.org && npm cache clean --force && mkdir project && chmod -R a+rX /opt/forge/deps\nWORKDIR /opt/forge/deps/project':
  'WORKDIR /opt/forge\nCOPY requirements.txt ./\nRUN python -m pip --isolated install --no-cache-dir --no-compile --no-deps --require-hashes --only-binary=:all: --index-url https://pypi.org/simple --disable-pip-version-check --target /opt/forge/python -r requirements.txt && chmod -R a+rX /opt/forge/python\nENV PYTHONPATH=/opt/forge/python\nWORKDIR /workspace';
 return `FROM ${base}\nLABEL dev.forgeide.runtime="${digest}"\nENV HOME=/tmp\n${install}\nUSER 1000:1000\n`;
}
export class DependencyRuntimes {
 private active?:AbortController;
 constructor(private state:string,private execute:typeof command=command){}
 cancel():void {this.active?.abort();}
 async plan(root:string,p:Proposal,profile:TestProfile,base:string):Promise<DependencyPlan> {
  const snap=await snapshot(root,p);try{const input=await dependencyInputs(snap.dir,profile,base);return {schema:1,profile,base,digest:input.digest,packages:input.packages,manifests:Object.keys(input.files),registry:profile==='node'?'registry.npmjs.org':'pypi.org / files.pythonhosted.org'};}finally{await fs.rm(snap.dir,{recursive:true,force:true});}
 }
 async prepare(root:string,p:Proposal,profile:TestProfile,base:string,expected:string):Promise<DependencyRuntime> {
  if(this.active)throw new Error('A dependency preparation is already running');
  const abort=this.active=new AbortController();let snap:Awaited<ReturnType<typeof snapshot>>|undefined,context:string|undefined;let built:string|undefined,retained=false;
  try {
   snap=await snapshot(root,p);const input=await dependencyInputs(snap.dir,profile,base);if(input.digest!==expected)throw new Error('Dependency inputs changed; review the new plan before downloading');
   await requireLocalDocker(this.execute);if(abort.signal.aborted)throw new Error('Preparation cancelled');
   const inspect=await this.execute('docker',['image','inspect',base,'--format','{{.Id}}'],10000,abort.signal);if(inspect.code!==0||inspect.timedOut)throw new Error('Prepare the base runtime first');
   context=await fs.mkdtemp(path.join(os.tmpdir(),'forge-dependencies-'));await fs.chmod(context,0o700);
   for(const [name,text] of Object.entries(input.files))await fs.writeFile(path.join(context,name),text,{mode:0o600});
   await fs.writeFile(path.join(context,'Dockerfile'),dependencyDockerfile(profile,base,input.digest),{mode:0o600});
   // The build context contains only the generated Dockerfile and validated manifests, never project code.
   const iid=path.join(context,'image-id');
   const result=await this.execute('docker',['build','--pull=false','--progress=plain','--network=default','--iidfile',iid,'--file',path.join(context,'Dockerfile'),context],180000,abort.signal);
   if(result.code!==0||result.timedOut||abort.signal.aborted)throw new Error(abort.signal.aborted?'Preparation cancelled':result.timedOut?'Dependency preparation timed out':'Dependency preparation failed: '+result.output);
   built=(await fs.readFile(iid,'utf8')).trim();if(!/^sha256:[a-f0-9]{64}$/.test(built))throw new Error('Docker did not return an immutable image ID');
   const record:DependencyRuntime={schema:1,image:built,profile,base,digest:input.digest,packages:input.packages,created:new Date().toISOString(),workdir:profile==='node'?'/opt/forge/deps/project':'/workspace'};
   const folder=path.join(this.state,'dependency-runtimes');await fs.mkdir(folder,{recursive:true,mode:0o700});const file=path.join(folder,built.slice(7)+'.json'),temp=file+'.'+randomUUID();await fs.writeFile(temp,JSON.stringify(record),{mode:0o600,flag:'wx'});await fs.rename(temp,file);retained=true;return record;
  } finally {
   if(built&&!retained)await this.execute('docker',['image','rm',built],10000).catch(()=>undefined);
   if(snap)await fs.rm(snap.dir,{recursive:true,force:true});if(context)await fs.rm(context,{recursive:true,force:true});this.active=undefined;
  }
 }
 async resolve(image:string,profile:TestProfile,dir:string):Promise<{workdir:string;digest:string}> {
  if(!/^sha256:[a-f0-9]{64}$/.test(image))throw new Error('Invalid prepared runtime ID');
  const file=path.join(this.state,'dependency-runtimes',image.slice(7)+'.json'),st=await fs.lstat(file);if(!st.isFile()||st.isSymbolicLink()||st.size>16384)throw new Error('Invalid prepared runtime record');
  const r=JSON.parse(await fs.readFile(file,'utf8')) as DependencyRuntime;
  if(r.schema!==1||r.image!==image||r.profile!==profile||r.workdir!==(profile==='node'?'/opt/forge/deps/project':'/workspace'))throw new Error('Runtime record does not match this profile');
  const current=await dependencyInputs(dir,profile,r.base);if(current.digest!==r.digest)throw new Error('Dependencies changed; prepare a new runtime for this exact lockfile');
  return {workdir:r.workdir,digest:r.digest};
 }
}
