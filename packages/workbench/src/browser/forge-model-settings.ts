import type {LanguageModel} from '@theia/ai-core/lib/common/language-model.js';
/** Permit only finite generation controls, never provider tools or arbitrary request fields. */
export function forgeModelSettings(model:Pick<LanguageModel,'vendor'>,settings:Record<string,unknown>={}):Record<string,number>|undefined {
 if(model.vendor?.toLowerCase()!=='ollama')return undefined;
 const result:Record<string,number>={num_ctx:8192,num_predict:2048,temperature:0.6,top_p:0.95,top_k:20};
 const bounds:Record<string,[number,number,boolean]>={num_ctx:[2048,32768,true],num_predict:[128,8192,true],temperature:[0,2,false],top_p:[0.01,1,false],top_k:[1,100,true]};
 for(const [name,[min,max,integer]] of Object.entries(bounds))if(settings[name]!==undefined){const n=settings[name];if(typeof n!=='number'||!Number.isFinite(n)||n<min||n>max||(integer&&!Number.isInteger(n)))throw Error('Forge Ollama '+name+' must be '+(integer?'an integer ':'')+'between '+min+' and '+max);result[name]=n;}
 return result;
}
