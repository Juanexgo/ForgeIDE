import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import type { ChangeGroup, GroupSummary, ProposedFileChange, Proposal } from '../common/forge-protocol.js';
export function enginePath():string {
 const exe=process.platform==='win32'?'forge-engine.exe':'forge-engine';
 const resources=(process as NodeJS.Process & {resourcesPath?:string}).resourcesPath;
 const choices=[process.env.FORGE_ENGINE_PATH,resources&&path.join(resources,'engine',exe),fileURLToPath(new URL('../../../engine/'+exe,import.meta.url)),fileURLToPath(new URL('../../../../engine/bin/'+exe,import.meta.url))];
 const found=choices.find((p):p is string=>!!p && fs.existsSync(p));
 if(!found)throw new Error('Forge Engine is missing. Build the Rust engine or reinstall the complete application.');return found;
}
export class GroupStore {
 constructor(readonly root:string,readonly state:string){}
 async request<T>(action:string,fields:object={}):Promise<T> {
  const input=JSON.stringify({version:1,root:this.root,state:this.state,action,...fields});
  if(Buffer.byteLength(input)>16*1024*1024)throw new Error('Transaction request exceeds 16 MiB');
  return new Promise((resolve,reject)=>{
   const child=spawn(enginePath(),[],{stdio:['pipe','pipe','pipe'],windowsHide:true});
   const chunks:Buffer[]=[];let size=0,stderr='',failed=false;
   const timer=setTimeout(()=>{failed=true;child.kill('SIGKILL');reject(new Error('Transaction timed out. Reopen its history to recover safely.'));},30000);
   child.stdout.on('data',(b:Buffer)=>{size+=b.length;if(size>17*1024*1024){failed=true;child.kill('SIGKILL');clearTimeout(timer);reject(new Error('Engine output limit exceeded'));}else chunks.push(b);});
   child.stderr.on('data',(b:Buffer)=>{stderr=(stderr+b.toString()).slice(-4000);});
   child.on('error',e=>{clearTimeout(timer);if(!failed){failed=true;reject(e);}});
   child.on('close',code=>{clearTimeout(timer);if(failed)return;try {const response=JSON.parse(Buffer.concat(chunks).toString('utf8'));if(!response.ok)throw new Error(response.error?.message||'Transaction rejected');if(code!==0)throw new Error('Engine exited unexpectedly');resolve(response.result as T);}catch(e){reject(e instanceof SyntaxError?new Error('Engine failed to return a valid response. '+stderr):e);}});
   child.stdin.on('error',()=>{});child.stdin.end(input);
  });
 }
 async draft(title:string,changes:ProposedFileChange[]):Promise<ChangeGroup> {
  if(typeof title!=='string' || !title.trim() || title.length>240 || !Array.isArray(changes) || !changes.length || changes.length>32)throw new Error('Provide a title and between 1 and 32 file changes');
  for(const c of changes)if(!c || typeof c.path!=='string' || !(c.after===null || typeof c.after==='string'))throw new Error('Each change needs a path and replacement text, or null for deletion');
  return this.request('draft',{group:{schema:1,id:randomUUID(),title:title.trim(),created:new Date().toISOString(),status:'review',changes:changes.map(c=>({path:c.path,before:null,after:c.after})),legacy:false,directories:[],recovery:null}});
 }
 list():Promise<GroupSummary[]> {return this.request('list');}
 get(id:string):Promise<ChangeGroup> {return this.request('get',{id});}
 apply(id:string):Promise<ChangeGroup> {return this.request('apply',{id});}
 revert(id:string):Promise<ChangeGroup> {return this.request('revert',{id});}
 recover():Promise<GroupSummary[]> {return this.request('recover');}
 async legacy(p:Proposal,revert:boolean):Promise<Proposal> {
  const groups=await this.list();
  if(!groups.some(g=>g.id===p.id))await this.request('create',{group:{schema:1,id:p.id,title:p.path,created:p.created,status:p.status,changes:[{path:p.path,before:p.before,after:p.after}],legacy:true,directories:[],recovery:null}});
  const g=await (revert?this.revert(p.id):this.apply(p.id));return {...p,status:g.status};
 }
}
