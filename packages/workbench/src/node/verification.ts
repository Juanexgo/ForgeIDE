import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import * as os from 'node:os';
import { createHash, randomUUID } from 'node:crypto';
import { constants } from 'node:fs';
import {findTool} from './tool-paths.js';
import { spawn } from 'node:child_process';
import { DatabaseSync } from 'node:sqlite';
import type { Proposal, RunReceipt } from '../common/forge-protocol.js';
export const sha = (s: string | Buffer): string => createHash('sha256').update(s).digest('hex');
export const proof = (p: Proposal): string => sha(JSON.stringify(p.changes?p.changes.map(c=>[c.path,c.before,c.after]):[p.path,p.before,p.after]));
const excluded = new Set(['node_modules','target','dist','build','vendor','coverage','__pycache__']);
const secret = (name: string): boolean => name.startsWith('.') || /^(id_rsa|id_ed25519|credentials|secrets|token)$/i.test(name) || /\.(pem|key|p12|pfx)$/i.test(name);
export interface Snapshot { dir: string; digest: string; files: number; bytes: number; }
/** No symlinks, dependencies, credentials, dotfiles or build outputs are copied. */
export async function snapshot(root: string, proposal: Proposal): Promise<Snapshot> {
 const changes=proposal.changes||[{path:proposal.path,before:proposal.before,after:proposal.after}];
 const map=new Map(changes.map(c=>[c.path,c]));const seen=new Set<string>();
 const dir=await fs.mkdtemp(path.join(os.tmpdir(),'forge-verify-'));await fs.chmod(dir,0o755);
 const entries:string[]=[];let files=0,bytes=0,visited=0;
 const safe=(p:string):boolean=>!!p && !path.isAbsolute(p) && !p.includes('\\') && p.split('/').every(s=>s && s!=='.' && s!=='..' && !secret(s) && !excluded.has(s));
 for(const c of changes)if(!safe(c.path)){await fs.rm(dir,{recursive:true,force:true});throw new Error('Proposed file is outside the verification profile: '+c.path);}
 const write=async(rel:string,data:Buffer,mode:number):Promise<void>=>{
  bytes+=data.length;if(++files>2000 || bytes>32*1024*1024)throw new Error('Snapshot exceeds limits (2000 files, 32 MiB)');
  const file=path.join(dir,rel);await fs.mkdir(path.dirname(file),{recursive:true,mode:0o755});await fs.writeFile(file,data,{mode,flag:'wx'});entries.push(rel+'\0'+sha(data)+'\0'+mode);
 };
 try {
  async function scan(from:string,relative:string):Promise<void>{
   for(const e of (await fs.readdir(from,{withFileTypes:true})).sort((a,b)=>a.name.localeCompare(b.name))){
    if(e.isSymbolicLink() || secret(e.name) || excluded.has(e.name))continue;
    const rel=relative?relative+'/'+e.name:e.name,full=path.join(from,e.name);
    if(e.isDirectory())await scan(full,rel);
    else if(e.isFile()){
     if(++visited>2000)throw new Error('Snapshot exceeds 2000 input files');
     const fd=await fs.open(full,constants.O_RDONLY|(constants.O_NOFOLLOW||0));let data:Buffer,mode:number;
     try {const st=await fd.stat();if(!st.isFile()||st.size>2*1024*1024)throw new Error('File exceeds snapshot limits');mode=st.mode&0o111?0o755:0o644;data=Buffer.alloc(st.size+1);const r=await fd.read(data,0,data.length,0);data=data.subarray(0,r.bytesRead);if(r.bytesRead!==st.size)throw new Error('File changed during snapshot');}finally{await fd.close();}
     const c=map.get(rel);
     if(c){seen.add(rel);if(c.before===null||!data.equals(Buffer.from(c.before)))throw new Error('Proposal baseline changed: '+rel);if(c.after===null)continue;data=Buffer.from(c.after);}
     await write(rel,data,mode);
    }
   }
  }
  await scan(root,'');
  for(const c of changes)if(!seen.has(c.path)){
   if(c.before!==null || c.after===null)throw new Error('Proposed input missing or excluded: '+c.path);
   try {await fs.lstat(path.join(root,c.path));throw new Error('Creation path now exists');}catch(e){if((e as NodeJS.ErrnoException).code!=='ENOENT')throw e;}
   await write(c.path,Buffer.from(c.after),0o644);
  }
  return {dir,digest:sha(entries.sort().join('\n')),files,bytes};
 }catch(e){await fs.rm(dir,{recursive:true,force:true});throw e;}
}
export class RunHistory {
 private db: DatabaseSync;
 constructor(dir: string) {
  this.db = new DatabaseSync(path.join(dir,'verification.sqlite'));
  this.db.exec('PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; CREATE TABLE IF NOT EXISTS runs (id TEXT PRIMARY KEY, proposal TEXT NOT NULL, created TEXT NOT NULL, receipt TEXT NOT NULL); PRAGMA user_version=1;');
 }
 save(r: RunReceipt): void {this.db.prepare('INSERT INTO runs VALUES(?,?,?,?)').run(r.id,r.proposalId,r.created,JSON.stringify(r));}
 list(): RunReceipt[] {return this.db.prepare('SELECT receipt FROM runs ORDER BY created DESC LIMIT 100').all().map(row=>JSON.parse(row.receipt as string) as RunReceipt);}
 close(): void {this.db.close();}
}
export interface CommandResult {code: number | null; output: string; timedOut: boolean;}
export function command(exe: string, args: string[], timeout: number, signal?: AbortSignal): Promise<CommandResult> {
 return new Promise((resolve,reject)=>{
  const child=spawn(findTool(exe)||exe,args,{stdio:['ignore','pipe','pipe'],windowsHide:true,env:{...process.env,DOCKER_CLI_HINTS:'false'}});
  let output='',timedOut=false,done=false;
  const collect=(b: Buffer):void=>{output+=b.toString();if(output.length>64000)output=output.slice(0,31000)+'\n[output truncated]\n'+output.slice(-32000);};
  child.stdout.on('data',collect);child.stderr.on('data',collect);
  let hard:ReturnType<typeof setTimeout>|undefined;
  const stop=():void=>{if(hard)return;child.kill('SIGTERM');hard=setTimeout(()=>child.kill('SIGKILL'),2000);};
  const timer=setTimeout(()=>{timedOut=true;stop();},timeout);
  const finish=():void=>{clearTimeout(timer);clearTimeout(hard);signal?.removeEventListener('abort',stop);};
  child.on('error',e=>{if(!done){done=true;finish();reject(e);}});
  child.on('close',code=>{if(!done){done=true;finish();resolve({code,output,timedOut});}});
  signal?.addEventListener('abort',stop,{once:true});if(signal?.aborted)stop();
 });
}
/** Reject remote daemon contexts: a remote bind mount would not be the verified local snapshot. */
export async function requireLocalDocker(execute:typeof command=command):Promise<void> {
 const local=(endpoint:string):boolean=>endpoint.startsWith('unix:///') || /^npipe:\/{2,4}\.\/pipe\//.test(endpoint);
 if(process.env.DOCKER_HOST && !local(process.env.DOCKER_HOST))throw new Error('Verification requires a local Docker daemon; remote DOCKER_HOST is not allowed');
 const result=await execute('docker',['context','inspect','--format','{{json .Endpoints.docker.Host}}'],5000);
 if(result.code!==0 || result.timedOut)throw new Error('Could not inspect Docker context');
 const endpoint=JSON.parse(result.output) as unknown;
 if(typeof endpoint!=='string' || !local(endpoint))throw new Error('Verification requires a local Docker context');
}
export class Verifier {
 private closing=false;
 private active = new Map<string,AbortController>();
 constructor(private history: RunHistory, private execute:typeof command=command, private runtimes?:{resolve(image:string,profile:'node'|'python',dir:string):Promise<{workdir:string;digest:string}>}) {}
 cancel(id: string): void {this.active.get(id)?.abort();}
 dispose(): void {this.closing=true;for(const c of this.active.values())c.abort();if(!this.active.size)this.history.close();}
 async run(root: string, p: Proposal, image: string, profile:'node'|'python'='node'): Promise<RunReceipt> {
  if(p.status!=='review')throw new Error('Only review proposals can be verified');
  if(!['node','python'].includes(profile)||(!(new RegExp('^'+profile+'@sha256:[0-9a-f]{64}$')).test(image)&&!(this.runtimes&&/^sha256:[a-f0-9]{64}$/.test(image)))) throw new Error('Use an official runtime pinned by digest or a prepared Forge dependency runtime');
  if(this.closing)throw new Error('Verifier is closed');
  if(this.active.size)throw new Error('A verification is already running in this workspace');
  const testCommand=profile==='python'?['python','-m','unittest','discover','-v']:['node','--test','--test-reporter=tap'];
  const id=randomUUID(),abort=new AbortController();this.active.set(p.id,abort);
  const start=performance.now();let snap: Snapshot | undefined;
  const receipt: RunReceipt={id,proposalId:p.id,proof:proof(p),created:new Date().toISOString(),status:'blocked',image,command:testCommand,profile,snapshot:'',files:0,bytes:0,exitCode:null,elapsedMs:0,output:''};
  const name='forge-verify-'+id;
  try {
   await requireLocalDocker(this.execute);
   const inspect=await this.execute('docker',['image','inspect',image,'--format','{{.Id}}'],10000,abort.signal);
   if(inspect.code!==0 || inspect.timedOut) {receipt.output='Docker or pinned image unavailable. '+inspect.output;return receipt;}
   snap=await snapshot(root,p);receipt.snapshot=snap.digest;receipt.files=snap.files;receipt.bytes=snap.bytes;
   let workdir='/workspace';if(image.startsWith('sha256:')){const prepared=await this.runtimes!.resolve(image,profile,snap.dir);workdir=prepared.workdir;receipt.dependencyProof=prepared.digest;}receipt.workdir=workdir;
   if(abort.signal.aborted){receipt.status='cancelled';return receipt;}
   const startup=performance.now();
   const container=await this.execute('docker',['run','--detach','--pull=never','--rm','--name',name,'--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=128','--memory=512m','--cpus=1','--user=1000:1000','--env=HOME=/tmp','--env=PYTHONDONTWRITEBYTECODE=1','--tmpfs=/tmp:rw,nosuid,noexec,size=64m','--mount','type=bind,source='+snap.dir+',target='+workdir+',readonly','--workdir='+workdir,image,'/bin/sleep','240'],120000,abort.signal);
   receipt.startupMs=Math.round(performance.now()-startup);
   if(container.code!==0||container.timedOut)throw new Error('Container startup failed before tests: '+container.output);
   if(abort.signal.aborted){receipt.status='cancelled';return receipt;}
   const execution=performance.now();
   const result=await this.execute('docker',['exec','--user=1000:1000','--workdir='+workdir,name,...testCommand],60000,abort.signal);
   receipt.executionMs=Math.round(performance.now()-execution);
   receipt.output=result.output;receipt.exitCode=result.code;
   const tests=Number((profile==='python'?result.output.match(/Ran (\d+) tests?\b/):result.output.match(/^# tests (\d+)$/m))?.[1]||0);
   const passes=profile==='python'?(/^OK(?: \(|\s*$)/m.test(result.output)?tests-Number(result.output.match(/skipped=(\d+)/)?.[1]||0):0):Number(result.output.match(/^# pass (\d+)$/m)?.[1]||0);
   receipt.status=abort.signal.aborted?'cancelled':result.timedOut?'timeout':result.code!==0?'failed':tests>0 && passes>0?'passed':'blocked';
   if(result.code===0 && receipt.status==='blocked')receipt.output+='\nNo passing tests were reported; this is not a verified change.';
  } catch(e) {receipt.output=e instanceof Error?e.message:String(e);receipt.status=abort.signal.aborted?'cancelled':'blocked';}
  finally {
   // The CLI may have been cancelled while its container kept running. Remove only our named container.
   await this.execute('docker',['rm','--force',name],5000).catch(()=>undefined);
   if(snap)await fs.rm(snap.dir,{recursive:true,force:true});
   this.active.delete(p.id);receipt.elapsedMs=Math.round(performance.now()-start);this.history.save(receipt);if(this.closing && !this.active.size)this.history.close();
  }
  return receipt;
 }
}
