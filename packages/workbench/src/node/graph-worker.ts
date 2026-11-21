import {parentPort} from 'node:worker_threads';
import {ProjectGraphIndex} from './project-graph.js';
let queue=Promise.resolve();const indexes=new Map<string,ProjectGraphIndex>();
parentPort?.on('message',({id,root}:{id:number;root:string})=>{queue=queue.then(async()=>{try{parentPort?.postMessage({id,result:await (()=>{let index=indexes.get(root);if(!index){if(indexes.size>=4)indexes.delete(indexes.keys().next().value!);index=new ProjectGraphIndex(root);indexes.set(root,index);}return index.run();})()});}catch(e){parentPort?.postMessage({id,error:e instanceof Error?e.message:String(e)});}});});
