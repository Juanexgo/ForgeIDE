import type { ForgeClient } from '../common/forge-protocol.js';
/** All windows connected to this backend must acknowledge the affected buffers. */
export class BufferCoordinator {
 private clients=new Map<string,{root:string;uri:string;client:ForgeClient}>();
 private queues=new Map<string,Promise<void>>();
 register(key:string,root:string,uri:string,client:ForgeClient):void {this.clients.set(key,{root,uri,client});}
 remove(prefix:string):void {for(const key of this.clients.keys())if(key.startsWith(prefix+':'))this.clients.delete(key);}
 async run<T>(root:string,paths:string[]|null,write:()=>Promise<T>):Promise<T> {
  const previous=this.queues.get(root)||Promise.resolve();let release!:()=>void;
  const current=new Promise<void>(resolve=>release=resolve);const tail=previous.then(()=>current);this.queues.set(root,tail);
  await previous;
  try {
   for(const {root:r,uri,client} of this.clients.values())if(r===root){
    let timer:ReturnType<typeof setTimeout>|undefined;
    try {const dirty=await Promise.race([client.checkBuffers(uri,paths),new Promise<never>((_,reject)=>timer=setTimeout(()=>reject(new Error('A window did not confirm its buffers. Close or reconnect it before applying.')),5000))]);if(dirty.length)throw new Error('Unsaved changes in another editor window: '+dirty.join(', '));}
    finally {if(timer)clearTimeout(timer);}
   }
   return await write();
  } finally {release();if(this.queues.get(root)===tail)this.queues.delete(root);}
 }
}
