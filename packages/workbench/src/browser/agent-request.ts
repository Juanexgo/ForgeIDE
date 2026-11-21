import {CancellationTokenSource,type CancellationToken} from '@theia/core/lib/common/cancellation.js';
import {ToolInvocationContext,type LanguageModelResponse,type ToolRequest} from '@theia/ai-core/lib/common/language-model.js';
const bytes=(s:string):number=>new TextEncoder().encode(s).length;
export async function agentRequest(execute:(tools:ToolRequest[],token:CancellationToken)=>Promise<LanguageModelResponse>,tools:ToolRequest[],parent:CancellationToken,id:string):Promise<LanguageModelResponse>{
 const source=new CancellationTokenSource(),read=new Set<string>();let calls=0,cleaned=false,proposed=false;
 const link=parent.onCancellationRequested(()=>source.cancel());if(parent.isCancellationRequested)source.cancel();
 const timer=setTimeout(()=>source.cancel(),180000);const cleanup=():void=>{if(cleaned)return;cleaned=true;clearTimeout(timer);link.dispose();source.dispose();};
 const guarded=tools.map(tool=>({...tool,handler:async(raw:string,ctx?:ToolInvocationContext)=>{
  if(cleaned||source.token.isCancellationRequested)throw Error('Forge request was cancelled');
  if(++calls>12){source.cancel();throw Error('Forge request exceeded 12 tool calls');}
  if(raw.length>65536||bytes(raw)>65536)throw Error('Tool input exceeds 64 KiB');
  const result=await tool.handler(raw,ctx);const serialized=JSON.stringify(result)??'';if(bytes(serialized)>128*1024)throw Error('Tool result exceeds 128 KiB; narrow the requested context');
  if(tool.id==='forge_draft_group'&&result&&typeof result==='object'&&'id' in result&&typeof result.id==='string')proposed=true;
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
  return '\n\nPropuesta de revisión creada: '+result.id+'. No aplicada ni verificada. Abre Forge Control para comparar los cambios.';
 };
 try{const result=await execute(guarded,source.token);
  if('stream' in result){const iterator=result.stream[Symbol.asyncIterator]();return {...result,stream:{async *[Symbol.asyncIterator](){let text='';try{while(true){const step=await iterator.next();if(step.done)break;const item=step.value;if('content' in item&&typeof item.content==='string'&&bytes(text)<65536)text+=item.content;yield item;}const summary=await fallback(text);if(summary)yield {content:summary};}finally{try{await iterator.return?.();}finally{cleanup();}}}}};}
  if('text' in result&&typeof result.text==='string')result.text+=await fallback(result.text);cleanup();return result;
 }catch(e){cleanup();throw e;}
}
