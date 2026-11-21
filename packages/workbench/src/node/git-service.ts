import {StringDecoder} from 'node:string_decoder';
import * as os from 'node:os';import {randomUUID} from 'node:crypto';
import {findTool} from './tool-paths.js';
import {spawn, type ChildProcess} from 'node:child_process';
import * as fs from 'node:fs';import * as path from 'node:path';import {createHash} from 'node:crypto';
import type {GitSnapshot,GitDiff,GitHubAccount,GitHubLogin} from '../common/forge-protocol.js';
const hash=(s:string):string=>createHash('sha256').update(s).digest('hex');
export function redact(s:string):string{return s.replace(/(https?:\/\/)[^\s/@]+(?::[^\s/@]*)?@/g,'$1[redacted]@').replace(/\bgithub_pat_[A-Za-z0-9_]{12,}\b/g,'[redacted]').replace(/\bgh[pousr]_[A-Za-z0-9_]{12,}\b/g,'[redacted]');}
export interface ExecResult {code:number;out:string;err:string;}
export function execute(exe:string,args:string[],input='',cwd?:string,timeout=30000):Promise<ExecResult>{return new Promise((resolve,reject)=>{
 const child=spawn(findTool(exe)||exe,args,{cwd,stdio:['pipe','pipe','pipe'],windowsHide:true,env:{...process.env,GIT_TERMINAL_PROMPT:'0',GIT_PAGER:'cat',PAGER:'cat'}});let out='',err='',bytes=0,done=false;const outDecoder=new StringDecoder('utf8'),errDecoder=new StringDecoder('utf8');
 const timer=setTimeout(()=>{child.kill('SIGKILL');},timeout);
 const collect=(which:'out'|'err',b:Buffer):void=>{bytes+=b.length;if(bytes>2*1024*1024){child.kill('SIGKILL');done=true;clearTimeout(timer);reject(new Error('Git output exceeded 2 MiB; narrow the operation'));return;}if(which==='out')out+=outDecoder.write(b);else err+=errDecoder.write(b);};
 child.stdout.on('data',b=>collect('out',b));child.stderr.on('data',b=>collect('err',b));child.stdin.on('error',()=>{});child.stdin.end(input);
 child.on('error',e=>{clearTimeout(timer);if(!done){done=true;reject(e);}});child.on('close',code=>{clearTimeout(timer);if(!done){done=true;resolve({code:code??-1,out:out+outDecoder.end(),err:redact(err+errDecoder.end())});}});
});}
export function parseStatus(text:string):GitSnapshot['files'] {
 const records=text.split('\0'),files:GitSnapshot['files']=[];
 for(let i=0;i<records.length;i++){
  const r=records[i];if(!r||r.startsWith('#'))continue;
  if(r.startsWith('? ')){files.push({path:r.slice(2),index:'?',worktree:'?',conflict:false,original:null});continue;}
  const fields=r[0]==='1'?8:r[0]==='2'?9:r[0]==='u'?10:0;if(!fields)continue;
  let start=0;for(let j=0;j<fields;j++)start=r.indexOf(' ',start)+1;
  const xy=r.split(' ',2)[1]||'..';files.push({path:r.slice(start),index:xy[0],worktree:xy[1],conflict:r[0]==='u',original:r[0]==='2'?(records[++i]||null):null});
 }return files;
}
export function splitDiff(text:string):GitDiff['hunks'] {
 const lines=text.split(/(?<=\n)/),out:GitDiff['hunks']=[];let current='';
 for(const line of lines){if(line.startsWith('@@ ')){if(current)out.push({index:out.length,header:current.split('\n')[0],patch:current});current=line;}else if(current)current+=line;}
 if(current)out.push({index:out.length,header:current.split('\n')[0],patch:current});return out;
}
export function sameDirectory(a:string,b:string):boolean {const left=fs.statSync(a,{bigint:true}),right=fs.statSync(b,{bigint:true});if(!left.isDirectory()||!right.isDirectory())return false;if(left.ino!==0n&&right.ino!==0n)return left.dev===right.dev&&left.ino===right.ino;return path.relative(fs.realpathSync.native(a),fs.realpathSync.native(b))==='';}
export class GitService {
 private trusted=false;
 private readonly hooks:string;
 constructor(readonly root:string,state:string,private exec=execute){this.hooks=path.join(state,'empty-hooks');fs.mkdirSync(this.hooks,{recursive:true,mode:0o700});}
 trust(value:boolean):void {this.trusted=value;}
 private allowed():void {if(!this.trusted)throw new Error('Enable actions for this trusted repository first');}
 private file(p:string):void {if(typeof p!=='string'||!p||path.isAbsolute(p)||p.includes('\\')||p.includes('\0')||p.split('/').some(x=>!x||x==='.'||x==='..'||x.toLowerCase()==='.git'))throw new Error('Use a relative repository file path');}
 private async git(args:string[],input='',hooks=false):Promise<string>{const base=['--no-pager','--literal-pathspecs','-c','core.fsmonitor=false','-c','protocol.ext.allow=never','-c','protocol.file.allow=never'];if(!hooks)base.push('-c','core.hooksPath='+this.hooks);const r=await this.exec('git',[...base,'-C',this.root,...args],input,this.root,120000);if(r.code!==0)throw new Error(redact(r.err||r.out||'Git failed'));return r.out;}
 private async branch(name:string):Promise<void>{if(typeof name!=='string'||!name||name.length>240||name.startsWith('-')||name.includes('\0'))throw new Error('Invalid branch');await this.git(['check-ref-format','--branch',name]);}
 async snapshot():Promise<GitSnapshot>{
  const top=(await this.git(['rev-parse','--show-toplevel'])).trim();if(!sameDirectory(top,this.root))throw new Error('Open the repository root to manage its complete Git state');
  const [status,refs,log,remote,names,email]=await Promise.all([this.git(['status','--porcelain=v2','--branch','-z','--untracked-files=normal','--ignore-submodules=all']),this.git(['for-each-ref','--format=%(refname:short)%00%(objectname)%00%(HEAD)','refs/heads']),this.git(['log','-30','--format=%H%x00%h%x00%an%x00%aI%x00%s','-z']).catch(()=>''),this.git(['remote','-v']),this.git(['config','--get','user.name']).catch(()=>''),this.git(['config','--get','user.email']).catch(()=>'')]);
  const head=status.match(/# branch\.head ([^\0]+)/)?.[1]||'(unknown)';const ab=status.match(/# branch\.ab \+(\d+) -(\d+)/);
  const commits=log.split('\0'),history:GitSnapshot['history']=[];for(let i=0;i+4<commits.length;i+=5)history.push({id:commits[i],short:commits[i+1],author:commits[i+2],date:commits[i+3],subject:commits[i+4]});
  return {branch:head,ahead:Number(ab?.[1]||0),behind:Number(ab?.[2]||0),files:parseStatus(status),branches:refs.trim()?refs.trim().split('\n').map(line=>{const [name,id,current]=line.split('\0');return {name,id,current:current==='*'};}):[],history,remotes:remote.trim()?remote.trim().split('\n').filter(l=>l.endsWith('(fetch)')).map(l=>{const [name,url]=l.split(/\s+/);return {name,url:redact(url)};}):[],identity:{name:names.trim(),email:email.trim()},trusted:this.trusted};
 }
 async init():Promise<void>{this.allowed();await this.git(['init']);}
 async identity(name:string,email:string):Promise<void>{this.allowed();if(!name.trim()||/[\r\n\0]/.test(name)||name.startsWith('-')||!/^[^\s@]+@[^\s@]+$/.test(email)||email.startsWith('-'))throw new Error('Provide a valid commit name and email');await this.git(['config','--local','user.name',name]);await this.git(['config','--local','user.email',email]);}
 async stage(p:string):Promise<void>{this.allowed();this.file(p);await this.git(['add','--',p]);}
 async unstage(p:string):Promise<void>{this.allowed();this.file(p);try{await this.git(['rev-parse','--verify','HEAD']);await this.git(['restore','--staged','--',p]);}catch(e){if((await this.git(['rev-parse','--verify','HEAD']).then(()=>true,()=>false)))throw e;await this.git(['rm','--cached','-f','--ignore-unmatch','--',p]);}}
 async diff(p:string,staged:boolean):Promise<GitDiff>{this.file(p);const patch=await this.git(['diff','--no-ext-diff','--no-textconv','--unified=3',...(staged?['--cached']:[]),'--',p]);return {path:p,staged,hash:hash(patch),patch,hunks:splitDiff(patch),binary:patch.includes('Binary files ')||patch.includes('GIT binary patch')};}
 async hunk(p:string,staged:boolean,index:number,expected:string):Promise<void>{this.allowed();const d=await this.diff(p,staged);if(d.hash!==expected)throw new Error('Diff changed; refresh before staging a hunk');if(d.binary||!Number.isInteger(index)||!d.hunks[index])throw new Error('Choose a valid text hunk');const header=d.patch.slice(0,/^@@ /m.exec(d.patch)?.index??0);if(/rename from |new file mode |deleted file mode /.test(header))throw new Error('Stage renames, creations and deletions as a complete file');const patch=header+d.hunks[index].patch;const args=['apply','--cached','--recount',...(staged?['--reverse']:[])];await this.git([...args,'--check','-'],patch);await this.git([...args,'-'],patch);}
 async commit(message:string,hooks:boolean):Promise<void>{this.allowed();if(!message.trim()||message.length>64000||message.includes('\0'))throw new Error('Write a commit message');await this.git(['commit','-F','-'],message,hooks);}
 async switchBranch(name:string,create:boolean):Promise<void>{this.allowed();await this.branch(name);await this.git(create?['switch','-c',name]:['switch','--',name]);}
 async merge(name:string,hooks:boolean):Promise<void>{this.allowed();await this.branch(name);await this.git(['merge','--no-edit',name], '',hooks);}
 async conflicts(p:string):Promise<{base:string;ours:string;theirs:string}>{this.file(p);const [base,ours,theirs]=await Promise.all([1,2,3].map(stage=>this.git(['show','--no-ext-diff','--no-textconv',':'+stage+':'+p]).catch(()=>'')));if([base,ours,theirs].some(s=>s.includes('\0')||s.length>1024*1024))throw new Error('Binary or oversized conflict');return {base,ours,theirs};}
 async network(action:'fetch'|'pull'|'push',remote:string):Promise<void>{this.allowed();const names=(await this.git(['remote'])).trim().split('\n');if(!names.includes(remote)||remote.startsWith('-'))throw new Error('Select a configured remote');const gh=['-c','credential.https://github.com.helper=!gh auth git-credential'];await this.git([...gh,...(action==='fetch'?['fetch','--prune',remote]:action==='pull'?['pull','--ff-only',remote]:['push','--set-upstream',remote,'HEAD'])]);}
 async worktreeAdd(name:string,branch:string):Promise<string>{this.allowed();if(!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,80}$/.test(name)||name==='.'||name==='..')throw new Error('Use a simple folder name for the new worktree');await this.branch(branch);const target=path.join(path.dirname(this.root),name);try{fs.lstatSync(target);throw new Error('Destination already exists');}catch(e){if((e as NodeJS.ErrnoException).code!=='ENOENT')throw e;}await this.git(['worktree','add','-b',branch,target]);return target;}
 async pullRequests():Promise<import('../common/forge-protocol.js').PullRequestInfo[]>{const r=await this.exec('gh',['pr','list','--limit','30','--json','number,title,state,headRefName,baseRefName,url'],'',this.root);if(r.code!==0)throw new Error(redact(r.err||'Connect GitHub first'));return JSON.parse(r.out);}
 async createPR(title:string,body:string,base:string,head:string):Promise<string>{this.allowed();if(!title.trim()||title.length>240||body.length>64000||/[\r\n\0]/.test(title)||body.includes('\0'))throw new Error('Provide a title and description');await this.branch(base);await this.branch(head);const r=await this.exec('gh',['pr','create','--title',title,'--body-file','-','--base',base,'--head',head],body,this.root,120000);if(r.code!==0)throw new Error(redact(r.err));return r.out.trim();}
 async worktrees():Promise<string>{return this.git(['worktree','list','--porcelain']);}
 async compare(from:string,to:string):Promise<string>{if(!/^[a-f0-9]{7,64}$/.test(from)||!/^[a-f0-9]{7,64}$/.test(to))throw new Error('Select commit hashes from history');return this.git(['diff','--no-ext-diff','--no-textconv',from,to,'--']);}
}
export class GitHubService {
 private child?:ChildProcess;private disabled=false;private readonly stateFile:string;
 private flow:GitHubLogin={status:'idle',code:'',url:'https://github.com/login/device',error:''};
 constructor(private exec=execute,stateFile?:string,private launch:typeof spawn=spawn,private ttl=15*60*1000){this.stateFile=stateFile||path.join(process.env.FORGE_STATE_DIR||path.join(os.homedir(),'.forgeide'),'github-account.json');try{const st=fs.lstatSync(this.stateFile);if(st.isSymbolicLink()||st.size>1024)throw Error('Invalid account state');this.disabled=JSON.parse(fs.readFileSync(this.stateFile,'utf8')).disconnected===true;}catch(e){if((e as NodeJS.ErrnoException).code!=='ENOENT')this.disabled=true;}}
 private save(disconnected:boolean):void{fs.mkdirSync(path.dirname(this.stateFile),{recursive:true,mode:0o700});const tmp=this.stateFile+'.'+randomUUID();fs.writeFileSync(tmp,JSON.stringify({schema:1,disconnected}),{mode:0o600,flag:'wx'});fs.renameSync(tmp,this.stateFile);this.disabled=disconnected;}
 private async inspect():Promise<GitHubAccount>{try{const r=await this.exec('gh',['api','user','--hostname','github.com','--jq','{login,html_url}']);if(r.code!==0)return {connected:false,cli:true,status:/401|bad credentials|expired/i.test(r.err)?'expired':'unavailable',error:redact(r.err)};const a=JSON.parse(r.out);if(typeof a.login!=='string'||!/^[a-zA-Z0-9][a-zA-Z0-9-]{0,38}$/.test(a.login))throw Error('Invalid account metadata');return {connected:true,cli:true,login:a.login,url:'https://github.com/'+a.login};}catch(e){return {connected:false,cli:(e as NodeJS.ErrnoException).code!=='ENOENT',status:'unavailable',error:'GitHub CLI is not connected. Sign in or install gh.'};}}
 async account():Promise<GitHubAccount>{return this.disabled?{connected:false,cli:true,status:'signed-out',error:'Cuenta desconectada de ForgeIDE; las credenciales de GitHub CLI permanecen en su almacén.'}:this.inspect();}
 async requireConnected():Promise<void>{if(!(await this.account()).connected)throw Error('Connect GitHub in Forge Control first');}
 async disconnect(expectedLogin:string):Promise<GitHubAccount>{const current=await this.account();if(current.connected&&current.login!==expectedLogin)throw Error('Account changed; refresh before disconnecting');this.cancel();this.save(true);return this.account();}
 async reconnect():Promise<GitHubAccount>{const current=await this.inspect();if(current.connected)this.save(false);return current;}
 start():GitHubLogin {
  if(this.child)return this.state();this.flow={status:'starting',code:'',url:'https://github.com/login/device',error:'',expiresAt:new Date(Date.now()+this.ttl).toISOString()};
  const child=this.child=this.launch(findTool('gh')||'gh',['auth','login','--hostname','github.com','--git-protocol','https','--web'],{stdio:['ignore','pipe','pipe'],windowsHide:true});let output='',closed=false;let hard:ReturnType<typeof setTimeout>|undefined;
  const timer=setTimeout(()=>{if(this.child!==child)return;this.flow.status='expired';this.flow.code='';this.flow.error='El inicio de sesión expiró. Inicia otro flujo para obtener un código nuevo.';child.kill('SIGTERM');hard=setTimeout(()=>child.kill('SIGKILL'),2000);},this.ttl);timer.unref();
  const read=(b:Buffer):void=>{if(this.child!==child||this.flow.status==='cancelled'||this.flow.status==='expired')return;output=(output+b.toString()).slice(-8000);const code=output.match(/code:\s*([A-Z0-9]{4}-[A-Z0-9]{4})/i)?.[1];if(code){this.flow.code=code;this.flow.status='waiting';}};
  const finish=(code:number|null,error?:string):void=>{if(closed)return;closed=true;clearTimeout(timer);if(hard)clearTimeout(hard);if(this.child!==child)return;this.child=undefined;this.flow.code='';if(['cancelled','expired'].includes(this.flow.status))return;
   if(code===0&&!error){try{this.save(false);this.flow.status='connected';}catch{this.flow.status='failed';this.flow.error='Could not persist the account connection';}}
   else{this.flow.status=/expired|timed out/i.test(output)?'expired':'failed';this.flow.error=error?redact(error):'Login did not complete. Retry in your browser or GitHub CLI.';}
  };
  child.stdout?.on('data',read);child.stderr?.on('data',read);child.on('error',e=>finish(-1,e.message));child.on('close',code=>finish(code));return this.state();
 }
 state():GitHubLogin{return {...this.flow};}
 cancel():void {if(!this.child)return;this.flow.status='cancelled';this.flow.code='';const child=this.child;child.kill('SIGTERM');const timer=setTimeout(()=>{if(this.child===child)child.kill('SIGKILL');},2000);timer.unref();}
}
