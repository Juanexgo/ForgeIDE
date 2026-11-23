import {AsyncLocalStorage} from 'node:async_hooks';
import {Ollama} from 'ollama';
import {OllamaModel} from '@theia/ai-ollama/lib/node/ollama-language-model.js';
import {createProxyFetch} from '@theia/ai-core/lib/node/proxy-util.js';
const decorated=new WeakSet<OllamaModel>();
const activeLocalRequests=new Map<string,number>();
const forgeAgents=new Set(['ForgeImplement','ForgeFix','ForgeTests','ForgeSecurity']);
/** Include model loading and HTTP response headers in cancellation/deadlines, not just generated chunks. */
export function deadlineOllama(model:OllamaModel,requestMs=180000,httpMs=90000):OllamaModel {
 if(decorated.has(model))return model;decorated.add(model);
 const scope=new AsyncLocalStorage<AbortSignal>();const original=model.request.bind(model);
 const internal=model as unknown as {host:()=>string|undefined;proxy?:string;model?:string;initializeOllama:()=>Ollama};
 internal.initializeOllama=()=>{const host=internal.host();if(!host)throw Error('Configure an Ollama host first');const base=createProxyFetch(internal.proxy)||globalThis.fetch;return new Ollama({host,fetch:(input,init)=>{const signal=scope.getStore();return base(input,{...init,signal:AbortSignal.any([...(init?.signal?[init.signal]:[]),...(signal?[signal]:[]),AbortSignal.timeout(httpMs)])});}});};
 model.request=async(request,token)=>{
  let local:URL|undefined;try{const url=new URL(internal.host()||'');if(['127.0.0.1','localhost','[::1]'].includes(url.hostname)&&['http:','https:'].includes(url.protocol))local=url;}catch{}
  const modelName=internal.model,key=local&&modelName?local.origin+'\n'+modelName:undefined;
  if(key)activeLocalRequests.set(key,(activeLocalRequests.get(key)||0)+1);
  let finished=false;
  const finish=async():Promise<void>=>{if(finished)return;finished=true;if(!key)return;const remaining=(activeLocalRequests.get(key)||1)-1;if(remaining){activeLocalRequests.set(key,remaining);return;}activeLocalRequests.delete(key);
   // Release only a configured loopback model after a Forge turn and only when no
   // application request still uses it. Downloaded weights remain installed.
   if(!forgeAgents.has(request.agentId||''))return;
   try{const response=await globalThis.fetch(new URL('/api/generate',local!),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({model:modelName,keep_alive:0,stream:false}),signal:AbortSignal.timeout(5000)});await response.body?.cancel();}catch{}
  };
  const controller=new AbortController();const subscription=token?.onCancellationRequested(()=>controller.abort());if(token?.isCancellationRequested)controller.abort();
  let cleaned=false;const cleanup=():void=>{if(cleaned)return;cleaned=true;clearTimeout(timer);subscription?.dispose();};
  const timer=setTimeout(()=>controller.abort(Error('Ollama request exceeded its time budget')),requestMs);controller.signal.addEventListener('abort',cleanup,{once:true});
  try{const result=await scope.run(controller.signal,()=>original(request,token));if('stream' in result){const iterator=result.stream[Symbol.asyncIterator]();return {...result,stream:{async *[Symbol.asyncIterator](){try{while(true){const step=await scope.run(controller.signal,()=>iterator.next());if(step.done)break;yield step.value;}}finally{try{await iterator.return?.();}finally{cleanup();await finish();}}}}};}cleanup();await finish();return result;}catch(e){cleanup();await finish();throw e;}
 };
 return model;
}
