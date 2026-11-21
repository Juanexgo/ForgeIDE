import {UpdatePanel} from './update-panel.js';
import * as React from 'react';
import {LanguagePanel} from './language-panel.js';
import { QualityPanel } from './quality-panel.js';
import { GraphView } from './graph-view.js';
import { GitPanel } from './git-panel.js';
import { GroupsPanel } from './groups-panel.js';
import { injectable, inject, postConstruct } from 'inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget.js';
import { WorkspaceService } from '@theia/workspace/lib/browser/workspace-service.js';
import { EditorManager } from '@theia/editor/lib/browser/editor-manager.js';
import { Saveable } from '@theia/core/lib/browser/saveable.js';
import { ReviewResources } from './review-resources.js';
import { DiffUris } from '@theia/core/lib/browser/diff-uris.js';
import URI from '@theia/core/lib/common/uri.js';
import { ForgeService, type Proposal, type Graph, type Health, type RunReceipt } from '../common/forge-protocol.js';
@injectable()
export class ForgeWidget extends ReactWidget {
 static readonly ID = 'forgeide.control';
 @inject(ForgeService) protected readonly api!: ForgeService;
 @inject(WorkspaceService) protected readonly workspace!: WorkspaceService;
 @inject(EditorManager) protected readonly editors!: EditorManager;
 @inject(ReviewResources) protected readonly memory!: ReviewResources;
 private session?: string; private root?: string;
 private tab: 'groups' | 'git' | 'review' | 'graph' | 'health' = 'groups'; private groupRevision=0; private pendingGroup?:string;
 private visited = new Set<string>(['groups']);
 private showTab(tab: typeof this.tab):void {this.tab=tab;this.visited.add(tab);this.update();}
 showGit():void {this.showTab('git');}
 private proposals: Proposal[] = []; private graph?: Graph; private health?: Health;
 private image=''; private runs:RunReceipt[]=[]; private verifying?:string;
 private error = ''; private busy = false; private file = ''; private after = ''; private selected = '';
 @postConstruct() protected init(): void {
  this.id = ForgeWidget.ID; this.title.label = 'Forge Control'; this.title.caption = 'SafeApply · Graph · Diagnostics';
  this.title.iconClass = 'codicon codicon-shield'; this.title.closable = true;
  this.addClass('forge-control'); this.node.tabIndex = 0; this.node.setAttribute('role','region'); this.node.setAttribute('aria-label','Forge Control');
  this.toDispose.push(this.workspace.onWorkspaceChanged(() => {this.session = undefined; this.root = undefined; this.graph = undefined; this.health = undefined; this.proposals = []; this.update();}));
  this.update();
 }
 protected onActivateRequest(): void {this.node.focus();}
 private async connect(): Promise<string> {
  const roots = await this.workspace.roots; const root = roots[0]?.resource.toString();
  if(!root) throw new Error('Abre una carpeta para usar Forge Control.');
  if(!this.session || root !== this.root) {this.session = await this.api.open(root); this.root = root;}
  return this.session;
 }
 private async run(fn: (session: string) => Promise<void>): Promise<void> {
  if(this.busy) return;
  this.busy = true; this.error = ''; this.update();
  try {await fn(await this.connect());} catch(e) {this.error = e instanceof Error ? e.message : String(e);}
  finally {this.busy = false; this.update();}
 }
 private refresh(): Promise<void> {return this.run(async s => {
  if(this.tab === 'groups')this.groupRevision++;
  if(this.tab === 'review') {const [proposals,runs]=await Promise.all([this.api.proposals(s),this.api.runs(s)]);this.proposals=proposals;this.runs=runs;}
  if(this.tab === 'graph') this.graph = await this.api.graph(s);
  if(this.tab === 'health') this.health = await this.api.health(s);
 });}
 private async diff(p: Proposal): Promise<void> {
  const left = new URI('forge-review:/' + p.id + '/before/' + p.path), right = new URI('forge-review:/' + p.id + '/after/' + p.path);
  this.memory.add(left, p.before); this.memory.add(right, p.after);
  await this.editors.open(DiffUris.encode(left, right, p.path + ' · SafeApply'));
 }
 private change(p: Proposal, revert: boolean, verified=false): Promise<void> {return this.run(async s => {
  const uri = new URI(this.root!).resolve(p.path);
  const open = this.editors.all.find(w => w.editor.uri.toString() === uri.toString());
  if(open && Saveable.get(open)?.dirty) throw new Error('Guarda o descarta los cambios pendientes de este archivo antes de continuar.');
  if(revert) await this.api.revert(s,p.id); else if(verified) await this.api.applyVerified(s,p.id); else await this.api.apply(s,p.id);
  this.proposals = await this.api.proposals(s);
 });}
 protected render(): React.ReactNode {
  const impacted = new Set<string>();
  if(this.tab==='graph'&&this.selected&&this.graph){const parents=new Map<string,string[]>();for(const e of this.graph.edges){const a=parents.get(e.to)||[];a.push(e.from);parents.set(e.to,a);}const queue=[this.selected];impacted.add(this.selected);for(let i=0;i<queue.length;i++)for(const from of parents.get(queue[i])||[])if(!impacted.has(from)){impacted.add(from);queue.push(from);}}
  return <div className="forge-page">
   <header className="forge-hero"><span className="forge-eyebrow">FORGEIDE / CONTROL CENTER</span><h1>Construye con confianza.</h1><p>Revisa cambios, explora dependencias y consulta el estado real de tu proyecto.</p><span className="forge-badge">0.9 · Theia Preview</span></header>
   <nav className="forge-tabs" aria-label="Herramientas Forge">{(['groups','git','review','graph','health'] as const).map(t => <button key={t} aria-pressed={this.tab===t} onClick={() => {this.showTab(t); void this.refresh();}}>{t==='git'?'Git y GitHub':t==='groups'?'SafeApply':t==='review'?'Historial 0.7':t==='graph'?'Project Graph':'Diagnostics'}</button>)}<button disabled={this.busy} onClick={() => void this.refresh()}>{this.busy?'Consultando…':'Actualizar'}</button></nav>
   {this.error && <p role="alert" className="forge-error">{this.error}</p>}
   {this.visited.has('git') && <div hidden={this.tab!=='git'}><GitPanel key={this.workspace.tryGetRoots()[0]?.resource.toString()||'none'} api={this.api} connect={()=>this.connect()} onProposal={id=>{this.pendingGroup=id;this.groupRevision++;this.showTab('groups');}} openPatch={async(name,text)=>{const uri=new URI('forge-review:/git/'+Date.now()).withPath('/git/'+Date.now()+'/'+name);this.memory.add(uri,text);await this.editors.open(uri);}}/></div>}
   {this.visited.has('groups') && <div hidden={this.tab!=='groups'}><GroupsPanel key={this.workspace.tryGetRoots()[0]?.resource.toString()||'none'} api={this.api} connect={()=>this.connect()} revision={this.groupRevision} initial={this.pendingGroup} diff={(g,c)=>this.diff({id:g.id,path:c.path,before:c.before||'',after:c.after||'',status:g.status,created:g.created,verification:'not_run'})}/></div>}
   {this.tab==='review' && <section><div className="forge-section-title"><h2>Cambios revisables</h2><span className="forge-muted">Persistentes · reversibles · controlados por ti</span></div>
    <p>Propón el contenido completo de un archivo existente. Revisa la comparación antes de aplicarlo. Puedes ejecutar pruebas Node en una copia filtrada, montada en modo solo lectura dentro de un contenedor sin red. El historial conserva el resultado y los archivos exactos probados.</p>
    <article className="forge-card"><h3>Verificación aislada · Node</h3><p>“Preparar runtime” descarga la imagen oficial Node 24. Las pruebas usan un snapshot sin dependencias, dotfiles ni claves conocidas; hasta 32 MiB. Solo disponen de /tmp para escribir, sin red, con límites de memoria y tiempo. Proyectos que requieren dependencias instaladas necesitan un perfil adicional.</p><button disabled={this.busy} onClick={() => void this.run(async s=>{this.image=await this.api.prepareRuntime(s);})}>Preparar runtime Node</button><label className="forge-label">Imagen inmutable<input value={this.image} placeholder="node@sha256:…" onChange={e=>{this.image=e.target.value;this.update();}} /></label></article>
    <label className="forge-label">Ruta relativa<input value={this.file} placeholder="src/main.ts" onChange={e => {this.file=e.target.value;this.update();}} /></label>
    <label className="forge-label">Contenido propuesto<textarea rows={7} spellCheck={false} value={this.after} onChange={e => {this.after=e.target.value;this.update();}} /></label>
    <button className="forge-primary" disabled={this.busy || !this.file} onClick={() => void this.run(async s => {const p=await this.api.propose(s,this.file,this.after);this.proposals=await this.api.proposals(s);await this.diff(p);})}>Crear propuesta y comparar</button>
    <div className="forge-cards">{this.proposals.length===0 && <div className="forge-empty">Todavía no hay propuestas. Abre una carpeta y crea la primera o actualiza el historial.</div>}{this.proposals.map(p => <article className="forge-card" key={p.id}><div><strong>{p.path}</strong><span className="forge-badge">{p.status}</span></div><p className="forge-muted">{new Date(p.created).toLocaleString()} · Pruebas: {this.runs.find(r=>r.proposalId===p.id)?.status || 'sin ejecutar'}</p><div className="forge-actions"><button onClick={() => void this.run(async () => this.diff(p))}>Comparar</button>{p.status==='review' && <button disabled={this.busy} onClick={() => void this.change(p,false)}>Aplicar sin pruebas</button>}{p.status==='review' && <><button disabled={this.busy || !this.image} onClick={() => void this.run(async s=>{this.verifying=p.id;this.update();try{await this.api.verify(s,p.id,this.image);this.runs=await this.api.runs(s);}finally{this.verifying=undefined;}})}>Ejecutar pruebas aisladas</button><button disabled={this.busy || this.runs.find(r=>r.proposalId===p.id)?.status!=='passed'} onClick={() => void this.change(p,false,true)}>Aplicar con verificación</button></>}{this.verifying===p.id && <button onClick={()=>{if(this.session)void this.api.cancelVerification(this.session,p.id);}}>Cancelar pruebas</button>}{p.status==='applied' && <button disabled={this.busy} onClick={() => void this.change(p,true)}>Revertir</button>}</div>{this.runs.find(r=>r.proposalId===p.id) && <details><summary>Recibo de verificación</summary><pre className="forge-output">{JSON.stringify(this.runs.find(r=>r.proposalId===p.id),null,2)}</pre></details>}</article>)}</div>
   </section>}
   {this.tab==='graph' && <section><h2>Dependencias e impacto</h2><p>Análisis estático JS/TS, Python, Go y Rust; alias, paquetes locales y servicios Compose. Selecciona un módulo para ver qué archivos dependen de él, incluso indirectamente. Las dependencias externas y los imports dinámicos no literales no se recorren.</p>{this.graph ? <><div className="forge-stats"><span>{this.graph.nodes.length} módulos</span><span>{this.graph.edges.length} conexiones</span><span>{this.graph.cycles?.length || 0} ciclos</span><span>{this.graph.unresolved} imports sin resolver</span><span>{this.graph.elapsedMs} ms</span>{this.graph.incremental&&<span>{this.graph.incremental.parsedFiles} analizados · {this.graph.incremental.reusedFiles} reutilizados</span>}</div>{this.graph.truncated && <p role="status">Se alcanzó el límite de análisis; el mapa es parcial.</p>}{this.graph.warnings?.map((w,i)=><p key={i} role="status">{w}</p>)}<GraphView graph={this.graph} selected={this.selected} onSelect={n=>{this.selected=n;this.update();}}/><details><summary>Ciclos de dependencias</summary>{this.graph.cycles?.length?<ul>{this.graph.cycles.map((c,i)=><li key={i}>{c.join(" → ")} → {c[0]}</li>)}</ul>:<p>No se encontraron ciclos en los imports resueltos; el análisis es parcial.</p>}</details><div className="forge-graph-layout"><div className="forge-file-list">{this.graph.nodes.map(n => <button key={n} aria-pressed={this.selected===n} onClick={() => {this.selected=n;this.update();}}>{n}</button>)}</div><article className="forge-card"><h3>{this.selected || 'Selecciona un módulo'}</h3>{this.selected && <><button onClick={() => void this.editors.open(new URI(this.root!).resolve(this.graph?.nodeDetails?.[this.selected]?.path||this.selected))}>Abrir archivo</button><h4>Impacto potencial ({Math.max(0,impacted.size-1)})</h4><ul>{[...impacted].filter(n=>n!==this.selected).map(n=><li key={n}>{n}</li>)}</ul><h4>Importa</h4><ul>{this.graph.edges.filter(e=>e.from===this.selected).map(e=><li key={e.to}>{e.to}</li>)}</ul></>}</article></div></>:<div className="forge-empty">Actualiza para analizar la primera carpeta del espacio de trabajo.</div>}</section>}
   {this.tab==='health' && <section><h2>Estado verificable</h2><UpdatePanel api={this.api}/><QualityPanel api={this.api} connect={()=>this.connect()}/><LanguagePanel api={this.api}/>{this.health ? <><div className="forge-stats"><span>{this.health.platform}</span><span>Node {this.health.node}</span><span>{this.health.memoryMB} MB · backend</span><span>{this.health.proposals} propuestas</span></div><p className="forge-path">{this.health.workspace}</p><article className="forge-card"><h3>Validación del proyecto</h3><p>{this.health.verification} Estas métricas describen el backend; los recibos de pruebas y avisos de dependencias se muestran por separado.</p></article></>:<div className="forge-empty">Actualiza para consultar el backend.</div>}<article className="forge-card"><h3>IA opcional</h3><p>Configura un proveedor en las preferencias de Theia AI. Forge no incluye credenciales. El proveedor recibirá el contexto que le compartas. Los agentes Forge Implement, Tests, Security y Fix pueden leer archivos, consultar el grafo y crear propuestas SafeApply; aplicar y revertir se realizan desde este panel.</p></article></section>}
  </div>;
 }
}
