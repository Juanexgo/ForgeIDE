import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import {readFileSync,readSync,statSync,lstatSync,openSync,fstatSync,closeSync,constants} from 'node:fs';
import ts from 'typescript';
import {parseDocument} from 'yaml';
import {parseImports,type ImportRecord,type ParseResult} from './native-parser.js';
import type {Graph} from '../common/forge-protocol.js';
const skipped=new Set(['node_modules','.git','.forgeide','target','dist','build','vendor','coverage','.next','__pycache__','.venv']);
const code=/\.(ts|tsx|mts|cts|js|jsx|mjs|cjs|py|go|rs)$/;
const js=/\.(ts|tsx|mts|cts|js|jsx|mjs|cjs)$/;
const compose=/^(?:docker-)?compose(?:[.-][a-zA-Z0-9_-]+)?\.ya?ml$/;
type Cache={fingerprint:string;imports:ImportRecord[];meta?:ParseResult['meta'];services?:Record<string,unknown>;errors?:boolean};
const fingerprint=(s:{ino:bigint;size:bigint;mtimeNs:bigint;ctimeNs:bigint}):string=>[s.ino,s.size,s.mtimeNs,s.ctimeNs].join(':');
export class ProjectGraphIndex {
 private cache=new Map<string,Cache>();
 constructor(private root:string){}
 async run():Promise<Graph>{
 const root=this.root,start=performance.now(),nodes:string[]=[],dirs=new Set<string>([path.resolve(root)]),jsonFiles=new Set<string>(),dataFiles=new Set<string>(),warnings:string[]=[];
 let truncated=false,visited=0,parsedFiles=0,reusedFiles=0,removedFiles=0,readBytes=0;
 async function scan(dir:string):Promise<void>{
  for(const e of (await fs.readdir(dir,{withFileTypes:true})).sort((a,b)=>a.name.localeCompare(b.name))){
   if(++visited>20000){truncated=true;return;}if(e.name.startsWith('.')||skipped.has(e.name)||e.isSymbolicLink())continue;
   const full=path.join(dir,e.name),rel=path.relative(root,full).split(path.sep).join('/');
   if(e.isDirectory()){dirs.add(path.resolve(full));await scan(full);}else if(e.isFile()){
    if(e.name.endsWith('.json'))jsonFiles.add(path.resolve(full));
    if(e.name==='Cargo.toml'||e.name==='go.mod'||compose.test(e.name)){if(dataFiles.size<128)dataFiles.add(rel);else truncated=true;}
    if(code.test(e.name)){if(nodes.length<1000)nodes.push(rel);else truncated=true;}
   }
  }
 }
 await scan(root);const sourceNodes=[...nodes],known=new Set(sourceNodes),native:{path:string;language:string;source:string}[]=[],pending=new Map<string,string>();
 const selected=new Set([...sourceNodes,...dataFiles]);for(const file of this.cache.keys())if(!selected.has(file)){this.cache.delete(file);removedFiles++;}
 for(const rel of selected){
  const file=path.join(root,rel),before=await fs.lstat(file,{bigint:true});if(!before.isFile()||before.isSymbolicLink()||before.size>512n*1024n){truncated=true;this.cache.delete(rel);continue;}
  const stamp=fingerprint(before);if(this.cache.get(rel)?.fingerprint===stamp){reusedFiles++;continue;}
  const fd=await fs.open(file,constants.O_RDONLY|(constants.O_NOFOLLOW||0));let source:string;
  try{const st=await fd.stat({bigint:true});if(st.ino!==before.ino||st.dev!==before.dev)throw Error('File changed while indexing: '+rel);const buf=Buffer.alloc(Number(st.size)+1);const r=await fd.read(buf,0,buf.length,0);const after=await fd.stat({bigint:true});if(r.bytesRead!==Number(st.size)||fingerprint(after)!==stamp)throw Error('File changed while indexing: '+rel);readBytes+=r.bytesRead;if(readBytes>16*1024*1024){truncated=true;this.cache.delete(rel);continue;}source=buf.subarray(0,r.bytesRead).toString('utf8');if(source.includes('\0')){truncated=true;this.cache.delete(rel);continue;}}finally{await fd.close();}
  parsedFiles++;
  if(js.test(rel)){
   const imports:ImportRecord[]=[];const tree=ts.createSourceFile(rel,source,ts.ScriptTarget.Latest,true);const visit=(n:ts.Node):void=>{if((ts.isImportDeclaration(n)||ts.isExportDeclaration(n))&&n.moduleSpecifier&&ts.isStringLiteral(n.moduleSpecifier))imports.push({kind:'module',spec:n.moduleSpecifier.text,names:[],level:0,scope:[]});if(ts.isCallExpression(n)&&(n.expression.kind===ts.SyntaxKind.ImportKeyword||ts.isIdentifier(n.expression)&&n.expression.text==='require')&&n.arguments[0]&&ts.isStringLiteral(n.arguments[0]))imports.push({kind:'module',spec:n.arguments[0].text,names:[],level:0,scope:[]});ts.forEachChild(n,visit);};visit(tree);this.cache.set(rel,{fingerprint:stamp,imports});
  }else if(compose.test(path.basename(rel))){
   try{const doc=parseDocument(source,{uniqueKeys:true});if(doc.errors.length)throw Error(doc.errors[0].message);const data=doc.toJS({maxAliasCount:20});if(!data||typeof data!=='object'||!data.services||Array.isArray(data.services)||typeof data.services!=='object')throw Error('Expected a Compose services mapping');this.cache.set(rel,{fingerprint:stamp,imports:[],services:data.services});}catch{warnings.push('Invalid or unsupported Compose document: '+rel);this.cache.delete(rel);truncated=true;}
  }else if(rel.endsWith('go.mod')){const module=/^module\s+(?:"([^"]+)"|([^\s]+))/m.exec(source);this.cache.set(rel,{fingerprint:stamp,imports:[],meta:{name:module?.[1]||module?.[2]}});}
  else{native.push({path:rel,language:rel.endsWith('.py')?'python':rel.endsWith('.go')?'go':rel.endsWith('.rs')?'rust':'cargo',source});pending.set(rel,stamp);}
 }
 if(native.length){try{for(const item of await parseImports(native)){if(!pending.has(item.path))continue;this.cache.set(item.path,{fingerprint:pending.get(item.path)!,imports:item.imports,meta:item.meta,errors:item.errors});}}catch(e){truncated=true;warnings.push(e instanceof Error?e.message:String(e));for(const file of pending.keys())this.cache.delete(file);}}
 for(const [file,c] of this.cache)if(c.errors)warnings.push('Syntax errors or parser limits; imports may be incomplete: '+file);
 const key=(p:string):string=>process.platform==='linux'?path.resolve(p):path.resolve(p).toLowerCase();
 const files=new Map(nodes.filter(n=>/\.(ts|tsx|mts|cts|js|jsx|mjs|cjs)$/.test(n)).map(n=>[key(path.join(root,n)),n]));const jsonKnown=new Set([...jsonFiles].map(key));const metadata=new Map<string,string>();let metadataBytes=0;
 const readJson=(p:string):string|undefined=>{p=path.resolve(p);if(!jsonKnown.has(key(p)))return undefined;if(metadata.has(key(p)))return metadata.get(key(p));try{const st=statSync(p);if(st.size>256*1024||metadata.size>=128||metadataBytes+st.size>8*1024*1024){truncated=true;return undefined;}const before=lstatSync(p);if(before.isSymbolicLink())return undefined;const fd=openSync(p,constants.O_RDONLY|(constants.O_NOFOLLOW||0));let text:string;try{const check=fstatSync(fd);if(check.ino!==before.ino||check.dev!==before.dev)return undefined;if(check.size>256*1024)return undefined;const buffer=Buffer.alloc(check.size+1);const length=readSync(fd,buffer,0,buffer.length,0);const after=fstatSync(fd);if(length!==check.size||after.size!==check.size||after.mtimeMs!==check.mtimeMs||after.ctimeMs!==check.ctimeMs)return undefined;text=buffer.subarray(0,length).toString('utf8');}finally{closeSync(fd);}metadataBytes+=st.size;metadata.set(key(p),text);return text;}catch{return undefined;}};
 const aliases=new Map<string,string>(),virtualDirs=new Set<string>();
 for(const p of jsonFiles)if(path.basename(p)==='package.json'){try{const pkg=JSON.parse(readJson(p)||'{}');if(typeof pkg.name==='string'&&/^(@[a-z0-9_.-]+\/)?[a-z0-9_.-]+$/.test(pkg.name)){const virtual=path.join(root,'node_modules',pkg.name);if(aliases.has(key(virtual))){aliases.delete(key(virtual));warnings.push('Duplicate workspace package: '+pkg.name);}else aliases.set(key(virtual),path.dirname(p));let dir=virtual;while(dir.startsWith(path.resolve(root))){virtualDirs.add(key(dir));if(dir===path.resolve(root))break;dir=path.dirname(dir);}}}catch{warnings.push('Invalid package metadata: '+path.relative(root,p));}}
 const actual=(p:string):string=>{const k=key(p);for(const [virtual,dir] of aliases)if(k===virtual||k.startsWith(virtual+path.sep))return path.join(dir,p.slice(virtual.length));return p;};
 const directories=new Set([...dirs].map(key));
 const host:ts.ModuleResolutionHost={fileExists:p=>files.has(key(actual(p)))||jsonKnown.has(key(actual(p))),readFile:p=>readJson(actual(p)),directoryExists:p=>directories.has(key(actual(p)))||virtualDirs.has(key(p)),realpath:p=>path.resolve(actual(p)),getCurrentDirectory:()=>root};
 const configs=[...jsonFiles].filter(p=>/^(tsconfig|jsconfig)\.json$/.test(path.basename(p))).sort((a,b)=>b.length-a.length);
 const options=new Map<string,ts.CompilerOptions>();
 function compiler(from:string):ts.CompilerOptions {const config=configs.find(p=>from.startsWith(path.dirname(p)+path.sep));if(!config)return {allowJs:true,moduleResolution:ts.ModuleResolutionKind.Node10};if(options.has(config))return options.get(config)!;const raw=ts.parseConfigFileTextToJson(config,readJson(config)||'{}');const parsed=ts.parseJsonConfigFileContent(raw.config||{},{useCaseSensitiveFileNames:process.platform==='linux',fileExists:host.fileExists,readFile:host.readFile,readDirectory:()=>nodes.map(n=>path.join(root,n))},path.dirname(config),undefined,config);const errors=parsed.errors.filter(e=>e.code!==18003);if(raw.error||errors.length){warnings.push('Some tsconfig options could not be resolved: '+path.relative(root,config));}options.set(config,parsed.options);return parsed.options;}
 const edges:Graph['edges']=[],edgeKeys=new Set<string>();let unresolved=0;const nodeDetails:NonNullable<Graph['nodeDetails']>={};
 const add=(from:string,to:string,kind?:'depends-on'|'contains'):void=>{const key=from+'\0'+to;if(edgeKeys.has(key))return;if(edges.length>=12000){truncated=true;return;}edgeKeys.add(key);edges.push(kind?{from,to,kind}:{from,to});};
 const one=(candidates:string[]):string|undefined=>candidates.find(n=>known.has(n));
 const nearest=(from:string,filename:string):string|undefined=>[...dataFiles].filter(n=>path.posix.basename(n)===filename&&(from.startsWith(path.posix.dirname(n)==='.'?'':path.posix.dirname(n)+'/'))).sort((a,b)=>b.length-a.length)[0];
 const crateEntry=(manifest:string):string|undefined=>{const c=this.cache.get(manifest)?.meta,dir=path.posix.dirname(manifest);return one([path.posix.join(dir,c?.lib||'src/lib.rs'),path.posix.join(dir,'src/main.rs')]);};
 const rustBase=(from:string,manifest:string|undefined):string=>{const entry=manifest?crateEntry(manifest):undefined;return entry?path.posix.dirname(entry):path.posix.dirname(from);};
 for(const from of sourceNodes){
  const language=js.test(from)?'typescript':from.endsWith('.py')?'python':from.endsWith('.go')?'go':'rust';nodeDetails[from]={kind:'file',path:from,language};
  for(const imp of this.cache.get(from)?.imports||[]){let resolved=false;const link=(to:string|undefined):void=>{if(to){add(from,to);resolved=true;}};
   if(js.test(from)){const r=ts.resolveModuleName(imp.spec,path.join(root,from),compiler(path.join(root,from)),host).resolvedModule;link(r?files.get(key(actual(r.resolvedFileName))):undefined);}
   else if(from.endsWith('.py')){
    const roots=imp.level?[path.posix.normalize(path.posix.join(path.posix.dirname(from),...Array.from({length:Math.max(0,imp.level-1)},()=> '..')))]:['','src'];
    for(const base of roots){const stem=path.posix.join(base,...imp.spec.split('.').filter(Boolean));link(one([stem+'.py',path.posix.join(stem,'__init__.py')]));if(imp.kind==='pyfrom')for(const name of imp.names)link(one([path.posix.join(stem,...name.split('.'))+'.py',path.posix.join(stem,...name.split('.'),'__init__.py')]));}
   }else if(from.endsWith('.go')){
    for(const manifest of dataFiles)if(manifest.endsWith('go.mod')){const name=this.cache.get(manifest)?.meta?.name;if(name&&(imp.spec===name||imp.spec.startsWith(name+'/'))){const dir=path.posix.join(path.posix.dirname(manifest),imp.spec.slice(name.length));for(const file of sourceNodes)if(file.endsWith('.go')&&!file.endsWith('_test.go')&&path.posix.dirname(file)===dir)link(file);}}
   }else{
    const manifest=nearest(from,'Cargo.toml'),base=rustBase(from,manifest),parts=imp.spec.replace(/\s/g,'').replace(/^::/,'').split('::').filter(Boolean);let start=base,segments:string[]=[];
    const fileModule=()=>/^(lib|main|mod)\.rs$/.test(path.posix.basename(from))?path.posix.dirname(from):from.slice(0,-3);
    if(imp.kind==='mod'){start=fileModule();segments=[...imp.scope,imp.spec];}
    else if(parts[0]==='crate'){segments=parts.slice(1);}
    else if(parts[0]==='self'){start=fileModule();segments=[...imp.scope,...parts.slice(1)];}
    else if(parts[0]==='super'){start=path.posix.join(fileModule(),...imp.scope);while(parts[0]==='super'){start=path.posix.dirname(start);parts.shift();}segments=parts;}
    else {const dep=manifest?this.cache.get(manifest)?.meta?.dependencies?.[parts[0]]:undefined;const target=dep&&manifest?path.posix.normalize(path.posix.join(path.posix.dirname(manifest),dep,'Cargo.toml')):undefined;if(target&&dataFiles.has(target)){start=rustBase(from,target);segments=parts.slice(1);link(crateEntry(target));}else{segments=parts;if(segments[0]===this.cache.get(manifest||'')?.meta?.name?.replace(/-/g,'_'))segments.shift();}}
    for(let length=segments.length;length>0&&!resolved;length--){const stem=path.posix.join(start,...segments.slice(0,length));link(one([stem+'.rs',path.posix.join(stem,'mod.rs')]));}
   }
   if(!resolved)unresolved++;
  }
 }
 let serviceCount=0;
 for(const file of dataFiles){const services=this.cache.get(file)?.services;if(!services)continue;const serviceIds=new Map<string,string>();for(const name of Object.keys(services)){if(!/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,63}$/.test(name)||serviceCount>=100){truncated=true;continue;}const id=file+'#service/'+name;nodes.push(id);serviceIds.set(name,id);nodeDetails[id]={kind:'service',path:file,language:'compose'};serviceCount++;}
  for(const [name,id] of serviceIds){const service=services[name] as {depends_on?:unknown;build?:unknown};if(!service||typeof service!=='object')continue;const depends=Array.isArray(service.depends_on)?service.depends_on:service.depends_on&&typeof service.depends_on==='object'?Object.keys(service.depends_on):[];
   for(const dep of depends){const to=typeof dep==='string'?serviceIds.get(dep):undefined;if(to)add(id,to,'depends-on');else{unresolved++;warnings.push('Unknown Compose dependency: '+String(dep));}}
   const context=typeof service.build==='string'?service.build:service.build&&typeof service.build==='object'?(service.build as {context?:unknown}).context:undefined;
   if(typeof context==='string'){const dir=path.posix.normalize(path.posix.join(path.posix.dirname(file),context));if(path.posix.isAbsolute(context)||dir==='..'||dir.startsWith('../')||context.includes('://')){warnings.push('External build context excluded: '+file+'/'+name);continue;}for(const source of sourceNodes)if(dir==='.'||source.startsWith(dir+'/'))add(id,source,'contains');}
  }
 }
 const adjacency=new Map(nodes.map(n=>[n,[] as string[]]));for(const e of edges)adjacency.get(e.from)!.push(e.to);
 const indices=new Map<string,number>(),low=new Map<string,number>(),stack:string[]=[],active=new Set<string>(),cycles:string[][]=[];let index=0;
 const visit=(v:string):void=>{indices.set(v,index);low.set(v,index++);stack.push(v);active.add(v);for(const w of adjacency.get(v)||[]){if(!indices.has(w)){visit(w);low.set(v,Math.min(low.get(v)!,low.get(w)!));}else if(active.has(w))low.set(v,Math.min(low.get(v)!,indices.get(w)!));}if(low.get(v)===indices.get(v)){const component:string[]=[];let w:string;do{w=stack.pop()!;active.delete(w);component.push(w);}while(w!==v);if(component.length>1||adjacency.get(v)?.includes(v))cycles.push(component.sort());}};for(const n of nodes)if(!indices.has(n))visit(n);
 return {nodes,edges,unresolved,cycles,nodeDetails,warnings:[...new Set(warnings)].slice(0,100),truncated,elapsedMs:Math.round(performance.now()-start),incremental:{parsedFiles,reusedFiles,removedFiles}};
 }
}
export function projectGraph(root:string):Promise<Graph>{return new ProjectGraphIndex(root).run();}
