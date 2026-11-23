import {CancellationTokenSource,type CancellationToken} from '@theia/core/lib/common/cancellation.js';
import {ToolInvocationContext,type LanguageModelResponse,type ToolRequest} from '@theia/ai-core/lib/common/language-model.js';
const bytes=(s:string):number=>new TextEncoder().encode(s).length;
export async function agentRequest(execute:(tools:ToolRequest[],token:CancellationToken)=>Promise<LanguageModelResponse>,tools:ToolRequest[],parent:CancellationToken,id:string,options:{handoffOnProposal?:boolean}={}):Promise<LanguageModelResponse>{
 const source=new CancellationTokenSource(),read=new Set<string>();let calls=0,cleaned=false,proposed=false,proposalPending=false,handoff=false,proposalId='';
 const link=parent.onCancellationRequested(()=>source.cancel());if(parent.isCancellationRequested)source.cancel();
 const timer=setTimeout(()=>source.cancel(),180000);const cleanup=():void=>{if(cleaned)return;cleaned=true;clearTimeout(timer);link.dispose();source.dispose();};
 const summary=():string=>'Propuesta de revisión creada: '+proposalId+'. No aplicada ni verificada. Revisa los cambios en Forge Control antes de ejecutar pruebas o aplicarlos.';
 const guarded=tools.map(tool=>({...tool,handler:async(raw:string,ctx?:ToolInvocationContext)=>{
  if(cleaned||source.token.isCancellationRequested)throw Error('Forge request was cancelled');
  if(++calls>12){source.cancel();throw Error('Forge request exceeded 12 tool calls');}
  if(raw.length>65536||bytes(raw)>65536)throw Error('Tool input exceeds 64 KiB');
  const isProposal=tool.id==='forge_draft_group'||tool.id==='forge_safeapply_propose';
  if(isProposal&&options.handoffOnProposal){if(proposalPending)throw Error('Create one review transaction per request');proposalPending=true;}
  let result:Awaited<ReturnType<ToolRequest['handler']>>;
  try{result=await tool.handler(raw,ctx);}catch(e){if(isProposal&&!proposed)proposalPending=false;throw e;}
  const serialized=JSON.stringify(result)??'';if(bytes(serialized)>128*1024)throw Error('Tool result exceeds 128 KiB; narrow the requested context');
  if(isProposal&&result&&typeof result==='object'&&'id' in result&&typeof result.id==='string'){
   proposed=true;proposalId=result.id;
   // A durable review transaction is the boundary of an editing turn. Stop provider
   // generation after it exists, preserving the real ID instead of waiting for loops.
   if(options.handoffOnProposal&&!source.token.isCancellationRequested&&!parent.isCancellationRequested){handoff=true;source.cancel();}
  }
  if(tool.id==='forge_read_file'&&result&&typeof result==='object'&&'path' in result&&'content' in result&&typeof result.path==='string'&&typeof result.content==='string')read.add(result.path);
  return result;
 }}));
 const fallback=async(text:string):Promise<string>=>{
  if(proposed)return '';
  const matches=[...text.matchAll(/```forge-changes\s*\n([\s\S]*?)\n```/g)];if(!matches.length)return '';
  if(matches.length!==1||bytes(matches[0][1])>65536)throw Error('Return exactly one bounded forge-changes object');
  const plan=JSON.parse(matches[0][1]) as {title?:unknown;changes?:unknown};if(typeof plan.title!=='string'||!Array.isArray(plan.changes)||!plan.changes.length||plan.changes.length>32)throw Error('Invalid structured review proposal');
  for(const change of plan.changes)if(!change||typeof change.path!=='string'||typeof change.after!=='string'||!read.has(change.path))throw Error('Structured fallback may edit only files successfully read during this request; creation/deletion requires the proposal tool');
  const tool=guarded.find(t=>t.id==='forge_draft_group');if(!tool)throw Error('No review tool available');const result=await tool.handler(JSON.stringify(plan),ToolInvocationContext.create('forge-review-'+id,source.token));
  if(!result||typeof result!=='object'||!('id' in result)||typeof result.id!=='string')throw Error('Review tool did not create a proposal');
  return '\n\n'+summary();
 };
 try{const result=await execute(guarded,source.token);
  if('stream' in result){const iterator=result.stream[Symbol.asyncIterator]();return {...result,stream:{async *[Symbol.asyncIterator](){let text='';try{
   try{while(true){const step=await iterator.next();if(step.done)break;const item=step.value;if('content' in item&&typeof item.content==='string'&&bytes(text)<65536)text+=item.content;yield item;}}catch(e){if(!handoff)throw e;}
   if(handoff)yield {content:'\n\n'+summary()};else {const extra=await fallback(text);if(extra)yield {content:extra};}
  }finally{try{await iterator.return?.();}catch(e){if(!handoff)throw e;}finally{cleanup();}}}}};}
  if(handoff){cleanup();return {text:summary()};}
  if('text' in result&&typeof result.text==='string')result.text+=await fallback(result.text);cleanup();return result;
 }catch(e){if(handoff){cleanup();return {text:summary()};}cleanup();throw e;}
}
