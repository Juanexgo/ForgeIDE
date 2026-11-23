import {draftGroupTool,verifyGroupTool} from '../common/forge-tool-definitions.js';
import {injectable,inject} from 'inversify';
import {WorkspaceService} from '@theia/workspace/lib/browser/workspace-service.js';
import {ToolProvider} from '@theia/ai-core/lib/common/tool-invocation-registry.js';
import type {ToolRequest} from '@theia/ai-core/lib/common/language-model.js';
import {ForgeService} from '../common/forge-protocol.js';
@injectable() abstract class WorkspaceTool implements ToolProvider {
 @inject(ForgeService) protected api!:ForgeService;@inject(WorkspaceService) protected workspace!:WorkspaceService;
 private root?:string;private id?:string;
 protected async session():Promise<string>{const root=(await this.workspace.roots)[0]?.resource.toString();if(!root)throw new Error('Open a local workspace');if(this.root!==root||!this.id){this.id=await this.api.open(root);this.root=root;}return this.id;}
 abstract getTool():ToolRequest;
}
@injectable() export class DraftGroupTool extends WorkspaceTool {getTool():ToolRequest{return draftGroupTool(this.api,()=>this.session());}}
@injectable() export class VerifyGroupTool extends WorkspaceTool {getTool():ToolRequest{return verifyGroupTool(this.api,()=>this.session());}}
export function bindGroupTools(bind:import('inversify').interfaces.Bind):void{for(const type of [DraftGroupTool,VerifyGroupTool]){bind(type).toSelf().inSingletonScope();bind(ToolProvider).toService(type);}}
