import {readFileTool,graphTool,runsTool} from '../common/forge-tool-definitions.js';
import { injectable, inject } from 'inversify';
import { WorkspaceService } from '@theia/workspace/lib/browser/workspace-service.js';
import { ToolProvider } from '@theia/ai-core/lib/common/tool-invocation-registry.js';
import type { ToolRequest } from '@theia/ai-core/lib/common/language-model.js';
import { ForgeService } from '../common/forge-protocol.js';
@injectable()
abstract class ForgeReadTool implements ToolProvider {
 @inject(ForgeService) protected api!:ForgeService;
 @inject(WorkspaceService) protected workspace!:WorkspaceService;
 private root?:string;private session?:string;
 protected async connect():Promise<string>{const root=(await this.workspace.roots)[0]?.resource.toString();if(!root)throw new Error('Open a local project');if(this.root!==root||!this.session){this.session=await this.api.open(root);this.root=root;}return this.session;}
 abstract getTool():ToolRequest;
}
@injectable() export class ReadFileTool extends ForgeReadTool {getTool():ToolRequest{return readFileTool(this.api,()=>this.connect());}}
@injectable() export class GraphTool extends ForgeReadTool {getTool():ToolRequest{return graphTool(this.api,()=>this.connect());}}
@injectable() export class RunsTool extends ForgeReadTool {getTool():ToolRequest{return runsTool(this.api,()=>this.connect());}}
export function bindReadTools(bind:import('inversify').interfaces.Bind):void {for(const type of [ReadFileTool,GraphTool,RunsTool]){bind(type).toSelf().inSingletonScope();bind(ToolProvider).toService(type);}}
