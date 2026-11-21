import {UpdateService,loadUpdatePolicy,loadAppVersion} from './update-service.js';
import {OllamaModel} from '@theia/ai-ollama/lib/node/ollama-language-model.js';
import {ConnectionContainerModule} from '@theia/core/lib/node/messaging/connection-container-module.js';
import {deadlineOllama} from './ollama-deadline.js';
import {DependencyRuntimes} from './dependency-runtime.js';
import type {TestProfile} from '../common/forge-protocol.js';
import { ContainerModule } from 'inversify';
import { ConnectionHandler } from '@theia/core/lib/common/messaging/handler.js';
import { RpcConnectionHandler } from '@theia/core/lib/common/messaging/proxy-factory.js';
import { fileURLToPath } from 'node:url';
import * as os from 'node:os';
import * as path from 'node:path';
import { randomUUID } from 'node:crypto';
import {languageTools} from './tool-paths.js';
import { QualityService } from './quality-service.js';
import { GitService, GitHubService } from './git-service.js';
import type { GitAction } from '../common/forge-protocol.js';
import { GroupStore } from './group-store.js';
import { BufferCoordinator } from './buffer-coordinator.js';
import type { ForgeClient, ChangeGroup, Proposal, ProposedFileChange } from '../common/forge-protocol.js';
import type { ForgeService } from '../common/forge-protocol.js';
import { FORGE_PATH } from '../common/forge-protocol.js';
import { SafeStore } from './safe-store.js';
import * as fs from 'node:fs/promises';
import { RunHistory, Verifier, proof, snapshot, command, requireLocalDocker } from './verification.js';
import { GraphWorker } from './graph-worker-client.js';
const graphWorker=new GraphWorker();
const github=new GitHubService();
export class ForgeBackend implements ForgeService {
 private updater?:Promise<UpdateService>;
 private updates() {return this.updater??=Promise.all([loadUpdatePolicy(),loadAppVersion()]).then(([policy,version])=>new UpdateService(policy,path.join(process.env.FORGE_STATE_DIR||path.join(os.homedir(),'.forgeide'),'updates'),version));}
 async checkUpdates() {return (await this.updates()).check();}
 async downloadUpdate(release:string) {return (await this.updates()).download(release);}
 async cancelUpdate() {(await this.updates()).cancel();}
 private dependencyRuntimes=new Map<string,DependencyRuntimes>();
 private audits=new Map<string,QualityService>();
 private gitServices=new Map<string,GitService>();
 private readonly connectionId=randomUUID();
 constructor(private coordinator=new BufferCoordinator(),private client?:ForgeClient){}
 private grouped=new Map<string,GroupStore>();
 private verifiers = new Map<string,{history:RunHistory;runner:Verifier}>();
 private sessions = new Map<string, SafeStore>();
 async open(uri: string): Promise<string> {
  const url = new URL(uri);
  if(url.protocol !== 'file:' || url.hostname) throw new Error('Forge tools currently require a local workspace');
  const canonical=await fs.realpath(fileURLToPath(url));
  for(const [id,s] of this.sessions)if(s.root===canonical)return id;
  if(this.sessions.size >= 16) throw new Error('Too many workspace sessions');
  const store = new SafeStore(fileURLToPath(url), process.env.FORGE_STATE_DIR || path.join(os.homedir(), '.forgeide', 'safeapply'));
  const id = randomUUID(); this.sessions.set(id, store);
  this.grouped.set(store.root,new GroupStore(store.root,store.dir));
  if(this.client)this.coordinator.register(this.connectionId+':'+id,store.root,uri,this.client);
  if(!this.verifiers.has(store.root)){const history=new RunHistory(store.dir);const dependencies=new DependencyRuntimes(store.dir);this.dependencyRuntimes.set(store.root,dependencies);this.verifiers.set(store.root,{history,runner:new Verifier(history,command,dependencies)});}
  return id;
 }
 private store(id: string): SafeStore { const s = this.sessions.get(id); if(!s) throw new Error('Workspace session expired'); return s; }
 private dependencies(id:string) {return this.dependencyRuntimes.get(this.store(id).root)!;}
 async planDependencies(id:string,gid:string,profile:TestProfile,base:string) {return this.dependencies(id).plan(this.store(id).root,this.proposal(await this.grouping(id).get(gid)),profile,base);}
 async prepareDependencies(id:string,gid:string,profile:TestProfile,base:string,expected:string) {return this.dependencies(id).prepare(this.store(id).root,this.proposal(await this.grouping(id).get(gid)),profile,base,expected);}
 async cancelDependencyPreparation(id:string) {this.dependencies(id).cancel();}
 async languageTools() {return languageTools();}
 private quality(id:string) {const s=this.store(id);let q=this.audits.get(s.root);if(!q){q=new QualityService(s.root,s.dir);this.audits.set(s.root,q);}return q;}
 async dependencyInventory(id:string) {return this.quality(id).inventory();}
 async scanDependencies(id:string,digest:string) {return this.quality(id).scan(digest);}
 async qualityReport(id:string) {return this.quality(id).latest();}
 async cancelDependencyScan(id:string) {this.quality(id).cancel();}
 private git(id:string) {const s=this.store(id);let git=this.gitServices.get(s.root);if(!git){git=new GitService(s.root,s.dir);this.gitServices.set(s.root,git);}return git;}
 async gitState(id:string) {return this.git(id).snapshot();}
 async gitTrust(id:string,value:boolean) {this.git(id).trust(value===true);}
 async gitDiff(id:string,p:string,staged:boolean) {return this.git(id).diff(p,staged);}
 async gitPullRequests(id:string) {await github.requireConnected();return this.git(id).pullRequests();}
 async gitWorktrees(id:string) {return this.git(id).worktrees();}
 async gitCompare(id:string,a:string,b:string) {return this.git(id).compare(a,b);}
 async githubDisconnect(login:string) {return github.disconnect(login);}
 async githubReconnect() {return github.reconnect();}
 async githubAccount() {return github.account();}
 async githubLogin() {return github.start();}
 async githubLoginState() {return github.state();}
 async githubCancelLogin() {github.cancel();}
 async gitAct(id:string,a:GitAction):Promise<{proposalId?:string;location?:string}> {
  const git=this.git(id);
  if(a.kind==='resolve') {const content=await git.conflicts(a.path);const g=await this.draftGroup(id,'Resolver conflicto: '+a.path,[{path:a.path,after:content[a.side]}]);return {proposalId:g.id};}
  return this.coordinator.run(this.store(id).root,(a.kind==='branch'||a.kind==='merge'||a.kind==='pull'||a.kind==='commit'&&a.hooks)?null:[],async()=>{
   switch(a.kind){case 'init':await git.init();break;case 'stage':await git.stage(a.path);break;case 'unstage':await git.unstage(a.path);break;
    case 'hunk':await git.hunk(a.path,a.staged,a.index,a.expected);break;case 'commit':await git.commit(a.message,a.hooks);break;
    case 'branch':await git.switchBranch(a.name,a.create);break;case 'merge':await git.merge(a.name,a.hooks);break;
    case 'worktree':return {location:await git.worktreeAdd(a.name,a.branch)};case 'pr':await github.requireConnected();return {location:await git.createPR(a.title,a.body,a.base,a.head)};
    case 'identity':await git.identity(a.name,a.email);break;case 'fetch':case 'pull':case 'push':await git.network(a.kind,a.remote);break;
    default:throw new Error('Unsupported Git action');}return {};
  });
 }
 private grouping(id:string) {return this.grouped.get(this.store(id).root)!;}
 private proposal(g:ChangeGroup):Proposal {const c=g.changes[0];return {id:g.id,path:c.path,before:c.before||'',after:c.after||'',status:g.status,created:g.created,verification:'not_run',changes:g.changes};}
 async groups(id:string) {return (await this.grouping(id).list()).filter(g=>!g.legacy);}
 async group(id:string,gid:string) {return this.grouping(id).get(gid);}
 async draftGroup(id:string,title:string,changes:ProposedFileChange[]) {return this.grouping(id).draft(title,changes);}
 async verifyGroup(id:string,gid:string,image:string,profile:'node'|'python'='node') {const g=await this.grouping(id).get(gid);return this.verification(id).runner.run(this.store(id).root,this.proposal(g),image,profile);}
 async applyGroup(id:string,gid:string,verified:boolean) {
  const g=await this.grouping(id).get(gid);const s=this.store(id);
  return this.coordinator.run(s.root,g.changes.map(c=>c.path),async()=>{
   if(verified){const p=this.proposal(g),r=this.verification(id).history.list().find(r=>r.proposalId===gid);if(!r||r.status!=='passed'||r.proof!==proof(p))throw new Error('Passing verification of this exact group is required');const input=await snapshot(s.root,p);try{if(input.digest!==r.snapshot)throw new Error('Workspace inputs changed after verification');}finally{await fs.rm(input.dir,{recursive:true,force:true});}}
   return this.grouping(id).apply(gid);
  });
 }
 async revertGroup(id:string,gid:string) {const g=await this.grouping(id).get(gid);return this.coordinator.run(this.store(id).root,g.changes.map(c=>c.path),()=>this.grouping(id).revert(gid));}
 private verification(id:string) {return this.verifiers.get(this.store(id).root)!;}
 async prepareRuntime(id:string,profile:'node'|'python'='node') {
  this.store(id);await requireLocalDocker();
  if(!['node','python'].includes(profile))throw new Error('Unknown runtime profile');
  const tag=profile==='python'?'python:3.13-alpine':'node:24-alpine';
  const pull=await command('docker',['pull',tag],120000);
  if(pull.code!==0 || pull.timedOut)throw new Error('Runtime download failed: '+pull.output);
  const result=await command('docker',['image','inspect',tag,'--format','{{json .RepoDigests}}'],10000);
  if(result.code!==0)throw new Error('Could not resolve image digest');
  const digest=(JSON.parse(result.output) as string[]).find(s=>new RegExp('^'+profile+'@sha256:[0-9a-f]{64}$').test(s));
  if(!digest)throw new Error('No immutable Node image digest found');return digest;
 }
 async runs(id:string) {return this.verification(id).history.list();}
 async verify(id:string, proposal:string, image:string) {
  const s=this.store(id),p=s.list().find(x=>x.id===proposal);if(!p)throw new Error('Proposal not found');
  return this.verification(id).runner.run(s.root,p,image);
 }
 async cancelVerification(id:string,proposal:string) {this.verification(id).runner.cancel(proposal);}
 async applyVerified(id:string,proposal:string) {
  const s=this.store(id),p=s.list().find(x=>x.id===proposal);if(!p)throw new Error('Proposal not found');
  const r=this.verification(id).history.list().find(x=>x.proposalId===proposal);
  if(!r || r.status!=='passed' || r.proof!==proof(p))throw new Error('A passing verification of this exact proposal is required');
  const current=await snapshot(s.root,p);
  try {if(current.digest!==r.snapshot)throw new Error('Workspace inputs changed after verification. Run tests again.');return this.apply(id,proposal);}
  finally {await fs.rm(current.dir,{recursive:true,force:true});}
 }
 async graph(id: string) {return graphWorker.run(this.store(id).root);}
 async health(id: string) {const s = this.store(id); return {platform: process.platform, node: process.versions.node, memoryMB: Math.round(process.memoryUsage().rss / 1048576), workspace: s.root, proposals:s.list().length, verification:this.verification(id).history.list().length+' verification receipts recorded. This is not a security certification.'};}
 async read(id:string,file:string) {return this.store(id).read(file);}
 async proposals(id: string) {const old=this.store(id).list();if(!old.length)return old;const groups=await this.grouping(id).list();return old.map(p=>({...p,status:groups.find(g=>g.id===p.id)?.status||p.status}));}
 async propose(id: string, file: string, after: string) {return this.store(id).propose(file, after);}
 async apply(id: string, proposal: string) {const s=this.store(id),p=(await this.proposals(id)).find(p=>p.id===proposal);if(!p)throw new Error('Proposal not found');return this.coordinator.run(s.root,[p.path],async()=>{const result=await this.grouping(id).legacy(p,false);s.recordState(p.id,result.status);return result;});}
 async revert(id: string, proposal: string) {const s=this.store(id),p=(await this.proposals(id)).find(p=>p.id===proposal);if(!p)throw new Error('Proposal not found');return this.coordinator.run(s.root,[p.path],async()=>{const result=await this.grouping(id).legacy(p,true);s.recordState(p.id,result.status);return result;});}
 dispose(): void {void this.updater?.then(u=>u.cancel()).catch(()=>{});for(const d of this.dependencyRuntimes.values())d.cancel();this.dependencyRuntimes.clear();for(const q of this.audits.values())q.cancel();this.audits.clear();this.coordinator.remove(this.connectionId);for(const v of this.verifiers.values()){v.runner.dispose();}this.verifiers.clear();this.grouped.clear();this.gitServices.clear();this.sessions.clear();}
}
export default new ContainerModule(bind => {
 bind(ConnectionContainerModule).toConstantValue(ConnectionContainerModule.create(({rebind})=>{rebind(OllamaModel).toSelf().inTransientScope().onActivation((_ctx,model)=>deadlineOllama(model));}));
 const coordinator=new BufferCoordinator();
 bind(ConnectionHandler).toDynamicValue(() => new RpcConnectionHandler<ForgeClient>(FORGE_PATH, proxy => {
  const service = new ForgeBackend(coordinator,proxy); proxy.onDidCloseConnection(() => service.dispose()); return service;
 })).inSingletonScope();
});
