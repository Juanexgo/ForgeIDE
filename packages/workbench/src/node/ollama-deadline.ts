import {AsyncLocalStorage} from 'node:async_hooks';
import {Ollama} from 'ollama';
import {OllamaModel} from '@theia/ai-ollama/lib/node/ollama-language-model.js';
import {createProxyFetch} from '@theia/ai-core/lib/node/proxy-util.js';
const decorated=new WeakSet<OllamaModel>();
/** Include model loading and HTTP response headers in cancellation/deadlines, not just generated chunks. */
export function deadlineOllama(model:OllamaModel,requestMs=180000,httpMs=90000):OllamaModel {
 if(decorated.has(model))return model;decorated.add(model);
 const scope=new AsyncLocalStorage<AbortSignal>();const original=model.request.bind(model);
 const internal=model as unknown as {host:()=>string|undefined;proxy?:string;initializeOllama:()=>Ollama};
 internal.initializeOllama=()=>{const host=internal.host();if(!host)throw Error('Configure an Ollama host first');const base=createProxyFetch(internal.proxy)||globalThis.fetch;return new Ollama({host,fetch:(input,init)=>{const signal=scope.getStore();return base(input,{...init,signal:AbortSignal.any([...(init?.signal?[init.signal]:[]),...(signal?[signal]:[]),AbortSignal.timeout(httpMs)])});}});};
 model.request=async(request,token)=>{
  const controller=new AbortController();const subscription=token?.onCancellationRequested(()=>controller.abort());if(token?.isCancellationRequested)controller.abort();
  let cleaned=false;const cleanup=():void=>{if(cleaned)return;cleaned=true;clearTimeout(timer);subscription?.dispose();};
  const timer=setTimeout(()=>controller.abort(Error('Ollama request exceeded its time budget')),requestMs);controller.signal.addEventListener('abort',cleanup,{once:true});
  try{const result=await scope.run(controller.signal,()=>original(request,token));if('stream' in result){const iterator=result.stream[Symbol.asyncIterator]();return {...result,stream:{async *[Symbol.asyncIterator](){try{while(true){const step=await scope.run(controller.signal,()=>iterator.next());if(step.done)break;yield step.value;}}finally{try{await iterator.return?.();}finally{cleanup();}}}}};}cleanup();return result;}catch(e){cleanup();throw e;}
 };
 return model;
}
