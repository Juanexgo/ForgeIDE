export interface UpdateArtifact {platform:string;arch:string;kind:'dmg'|'exe'|'deb'|'AppImage'|'zip';url:string;sha256:string;bytes:number;}
export interface UpdateStatus {status:'available'|'current'|'unsupported'|'downloaded';current:string;release?:string;message:string;artifact?:UpdateArtifact;path?:string;}
export type TestProfile = 'node'|'python';
export interface DependencyPlan {schema:1;profile:TestProfile;base:string;digest:string;packages:number;manifests:string[];registry:string;}
export interface DependencyRuntime {schema:1;image:string;profile:TestProfile;base:string;digest:string;packages:number;created:string;workdir:string;}
export const ForgeService = Symbol('ForgeService');
export const FORGE_PATH = '/services/forgeide';
export interface Proposal { id: string; path: string; before: string; after: string; status: 'review' | 'applying' | 'applied' | 'reverting' | 'reverted' | 'conflict'; created: string; verification: 'not_run'; changes?: FileChange[]; }
export interface RunReceipt {startupMs?:number;executionMs?:number;dependencyProof?:string;workdir?:string;id:string;proposalId:string;proof:string;created:string;status:'passed'|'failed'|'blocked'|'timeout'|'cancelled';image:string;command:string[];profile?:'node'|'python';snapshot:string;files:number;bytes:number;exitCode:number|null;elapsedMs:number;output:string;}
export interface Graph { nodes: string[]; edges: { from: string; to: string; kind?:'depends-on'|'contains' }[]; nodeDetails?:Record<string,{kind:'file'|'service';path:string;language:string}>; incremental?:{parsedFiles:number;reusedFiles:number;removedFiles:number}; unresolved: number; cycles?: string[][];warnings?:string[]; truncated: boolean; elapsedMs: number; }
export interface Health { platform: string; node: string; memoryMB: number; workspace: string; proposals: number; verification: string; }
export interface ForgeService {
 checkUpdates():Promise<UpdateStatus>;
 downloadUpdate(expectedRelease:string):Promise<UpdateStatus>;
 cancelUpdate():Promise<void>;
 planDependencies(session:string,group:string,profile:TestProfile,base:string):Promise<DependencyPlan>;
 prepareDependencies(session:string,group:string,profile:TestProfile,base:string,expectedDigest:string):Promise<DependencyRuntime>;
 cancelDependencyPreparation(session:string):Promise<void>;
 languageTools():Promise<{language:string;tools:{name:string;path:string|null}[];extension:string}[]>;
 dependencyInventory(session:string):Promise<DependencyInventory>;
 scanDependencies(session:string,digest:string):Promise<QualityReport>;
 qualityReport(session:string):Promise<QualityReport|null>;
 cancelDependencyScan(session:string):Promise<void>;
 gitState(session:string):Promise<GitSnapshot>;
 gitTrust(session:string,trusted:boolean):Promise<void>;
 gitDiff(session:string,path:string,staged:boolean):Promise<GitDiff>;
 gitAct(session:string,action:GitAction):Promise<{proposalId?:string;location?:string}>;
 gitPullRequests(session:string):Promise<PullRequestInfo[]>;
 gitWorktrees(session:string):Promise<string>;
 gitCompare(session:string,from:string,to:string):Promise<string>;
 githubDisconnect(expectedLogin:string):Promise<GitHubAccount>;
 githubReconnect():Promise<GitHubAccount>;
 githubAccount():Promise<GitHubAccount>;
 githubLogin():Promise<GitHubLogin>;
 githubLoginState():Promise<GitHubLogin>;
 githubCancelLogin():Promise<void>;
 groups(session:string):Promise<GroupSummary[]>;
 group(session:string,id:string):Promise<ChangeGroup>;
 draftGroup(session:string,title:string,changes:ProposedFileChange[]):Promise<ChangeGroup>;
 applyGroup(session:string,id:string,verified:boolean):Promise<ChangeGroup>;
 revertGroup(session:string,id:string):Promise<ChangeGroup>;
 verifyGroup(session:string,id:string,image:string,profile?:'node'|'python'):Promise<RunReceipt>;
 open(rootUri: string): Promise<string>;
 graph(session: string): Promise<Graph>;
 health(session: string): Promise<Health>;
 read(session:string,path:string):Promise<string>;
 prepareRuntime(session:string,profile?:'node'|'python'):Promise<string>;
 runs(session:string): Promise<RunReceipt[]>;
 verify(session:string,id:string,image:string,profile?:'node'|'python'):Promise<RunReceipt>;
 cancelVerification(session:string,id:string):Promise<void>;
 applyVerified(session:string,id:string):Promise<Proposal>;
 proposals(session: string): Promise<Proposal[]>;
 propose(session: string, path: string, after: string): Promise<Proposal>;
 apply(session: string, id: string): Promise<Proposal>;
 revert(session: string, id: string): Promise<Proposal>;
}
export type GroupStatus=Proposal['status'];
export interface ProposedFileChange {path:string;after:string|null;}
export interface FileChange extends ProposedFileChange {before:string|null;mode?:number|null;}
export interface ChangeGroup {schema:1;id:string;title:string;created:string;status:GroupStatus;changes:FileChange[];legacy:boolean;directories:string[];recovery:string|null;}
export interface GroupSummary {id:string;title:string;created:string;status:GroupStatus;legacy:boolean;recovery:string|null;files:{path:string;kind:'create'|'edit'|'delete'}[];}

export interface ForgeClient {checkBuffers(rootUri:string,paths:string[]|null):Promise<string[]>;}
export interface GitSnapshot {branch:string;ahead:number;behind:number;files:{path:string;index:string;worktree:string;conflict:boolean;original:string|null}[];branches:{name:string;id:string;current:boolean}[];history:{id:string;short:string;author:string;date:string;subject:string}[];remotes:{name:string;url:string}[];identity:{name:string;email:string};trusted:boolean;}
export interface GitDiff {path:string;staged:boolean;hash:string;patch:string;hunks:{index:number;header:string;patch:string}[];binary:boolean;}
export interface GitHubAccount {connected:boolean;cli:boolean;status?:'expired'|'signed-out'|'unavailable';login?:string;url?:string;error?:string;}
export interface GitHubLogin {status:'idle'|'starting'|'waiting'|'connected'|'failed'|'cancelled'|'expired';expiresAt?:string;code:string;url:string;error:string;}
export type GitAction={kind:'init'}|{kind:'stage'|'unstage';path:string}|{kind:'hunk';path:string;staged:boolean;index:number;expected:string}|{kind:'commit';message:string;hooks:boolean}|{kind:'branch';name:string;create:boolean}|{kind:'merge';name:string;hooks:boolean}|{kind:'identity';name:string;email:string}|{kind:'fetch'|'pull'|'push';remote:string}|{kind:'resolve';path:string;side:'ours'|'theirs'}|{kind:'worktree';name:string;branch:string}|{kind:'pr';title:string;body:string;base:string;head:string};

export interface PullRequestInfo {number:number;title:string;state:string;headRefName:string;baseRefName:string;url:string;}
export interface DependencyInventory {digest:string;sources:{path:string;sha256:string}[];packages:{name:string;version:string;ecosystem:'npm'|'crates.io'|'PyPI'}[];notes:string[];}
export interface Vulnerability {id:string;name:string;version:string;ecosystem:string;severity:string;title:string;url:string;}
export interface QualityReport {created:string;digest:string;sources:DependencyInventory['sources'];scanned:number;findings:Vulnerability[];notes:string[];stale:boolean;}
