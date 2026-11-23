import {proposalTool} from '../common/forge-tool-definitions.js';
import { CommandRegistry } from '@theia/core/lib/common/command.js';
import { KeybindingRegistry } from '@theia/core/lib/browser/keybinding.js';
import { ContainerModule, injectable, inject } from 'inversify';
import { WidgetFactory } from '@theia/core/lib/browser/widget-manager.js';
import { AbstractViewContribution, bindViewContribution } from '@theia/core/lib/browser/shell/view-contribution.js';
import { FrontendApplicationContribution } from '@theia/core/lib/browser/frontend-application-contribution.js';
import { ServiceConnectionProvider } from '@theia/core/lib/browser/messaging/service-connection-provider.js';
import { WorkspaceService } from '@theia/workspace/lib/browser/workspace-service.js';
import { ToolProvider } from '@theia/ai-core/lib/common/tool-invocation-registry.js';
import type { ToolRequest } from '@theia/ai-core/lib/common/language-model.js';
import { Saveable } from '@theia/core/lib/browser/saveable.js';
import { EditorManager } from '@theia/editor/lib/browser/editor-manager.js';
import URI from '@theia/core/lib/common/uri.js';
import { ForgeService, FORGE_PATH } from '../common/forge-protocol.js';
import { ResourceResolver } from '@theia/core/lib/common/resource.js';
import { ReviewResources } from './review-resources.js';
import { bindGroupTools } from './group-tools.js';
import { bindForgeAgents } from './forge-agents.js';
import { bindReadTools } from './forge-read-tools.js';
import { ForgeWidget } from './forge-widget.js';
import '../../src/browser/style/forge.css';
@injectable()
class ForgeContribution extends AbstractViewContribution<ForgeWidget> implements FrontendApplicationContribution {
 constructor() {super({widgetId:ForgeWidget.ID,widgetName:'Forge Control',defaultWidgetOptions:{area:'main'},toggleCommandId:'forgeide.control.open',toggleKeybinding:'ctrlcmd+alt+f'});}
 override registerCommands(registry:CommandRegistry):void {super.registerCommands(registry);registry.registerCommand({id:'forgeide.git.open',label:'Forge: Git y GitHub'},{execute:async()=>{const w=await this.openView({activate:true});w.showGit();}});}
 override registerKeybindings(registry:KeybindingRegistry):void {super.registerKeybindings(registry);registry.registerKeybinding({command:'forgeide.git.open',keybinding:'ctrlcmd+alt+g'});}
 async initializeLayout(): Promise<void> {await this.openView({activate:true});}
}
@injectable()
class ForgeProposalTool implements ToolProvider {
 @inject(ForgeService) protected api!: ForgeService;
 @inject(WorkspaceService) protected workspace!: WorkspaceService;
 private root?: string; private session?: string;
 getTool():ToolRequest {return proposalTool(this.api,async()=>{
  const root=(await this.workspace.roots)[0]?.resource.toString();if(!root)throw Error('Open a local workspace first');
  if(root!==this.root||!this.session){this.session=await this.api.open(root);this.root=root;}return this.session;
 });}
}
export default new ContainerModule(bind => {
 bindForgeAgents(bind);bindReadTools(bind);bindGroupTools(bind);
 bind(ForgeService).toDynamicValue(ctx => ServiceConnectionProvider.createProxy<ForgeService>(ctx.container, FORGE_PATH,{checkBuffers:async(root:string,paths:string[]|null)=>{const targets=paths&&new Set(paths.map(p=>new URI(root).resolve(p).toString()));return ctx.container.get(EditorManager).all.filter(w=>(targets?targets.has(w.editor.uri.toString()):w.editor.uri.toString().startsWith(new URI(root).toString().replace(/\/$/,'')+'/'))&&Saveable.get(w)?.dirty).map(w=>w.editor.uri.path.toString());}})).inSingletonScope();
 bind(ReviewResources).toSelf().inSingletonScope(); bind(ResourceResolver).toService(ReviewResources);
 bind(ForgeWidget).toSelf();
 bind(WidgetFactory).toDynamicValue(ctx => ({id:ForgeWidget.ID,createWidget:()=>ctx.container.get(ForgeWidget)})).inSingletonScope();
 bindViewContribution(bind,ForgeContribution);
 bind(FrontendApplicationContribution).toService(ForgeContribution);
 bind(ForgeProposalTool).toSelf().inSingletonScope(); bind(ToolProvider).toService(ForgeProposalTool);
});
