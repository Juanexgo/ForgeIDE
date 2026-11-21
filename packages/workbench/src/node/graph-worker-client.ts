import {fileURLToPath,pathToFileURL} from 'node:url';
import {Worker} from 'node:worker_threads';import {existsSync} from 'node:fs';
import type {Graph} from '../common/forge-protocol.js';
export class GraphWorker {
 private worker?:Worker;private counter=0;private jobs=new Map<number,{resolve:(g:Graph)=>void;reject:(e:Error)=>void;timer:ReturnType<typeof setTimeout>}>();
 private get():Worker {if(this.worker)return this.worker;const packed=new URL('./forge-graph-worker.js',import.meta.url);const unpacked=fileURLToPath(packed).replace(/app\.asar([/\\])/,'app.asar.unpacked$1');const script=existsSync(unpacked)?pathToFileURL(unpacked):existsSync(packed)?packed:new URL('./graph-worker.js',import.meta.url);const w=this.worker=new Worker(script,{resourceLimits:{maxOldGenerationSizeMb:192}});w.unref();w.on('message',m=>{const job=this.jobs.get(m.id);if(!job)return;this.jobs.delete(m.id);clearTimeout(job.timer);if(m.error)job.reject(new Error(m.error));else job.resolve(m.result);if(!this.jobs.size)w.unref();});w.on('error',e=>this.stop(e.message));w.on('exit',code=>{if(this.worker===w)this.stop('Graph worker exited ('+code+')');});return w;}
 run(root:string):Promise<Graph>{if(this.jobs.size>=4)return Promise.reject(new Error('Graph indexing queue is full'));return new Promise((resolve,reject)=>{const id=++this.counter,w=this.get();w.ref();const timer=setTimeout(()=>this.stop('Graph indexing timed out; narrow the workspace'),30000);this.jobs.set(id,{resolve,reject,timer});w.postMessage({id,root});});}
 stop(message='Graph indexing cancelled'):void {const w=this.worker;this.worker=undefined;for(const j of this.jobs.values()){clearTimeout(j.timer);j.reject(new Error(message));}this.jobs.clear();void w?.terminate();}
}
