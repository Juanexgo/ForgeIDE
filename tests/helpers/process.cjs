// Wait for the owned adapter/server to release handles before removing its fixture.
async function stopChild(child){
 if(child.exitCode!==null||child.signalCode!==null)return;
 let timer;
 const closed=new Promise((resolve,reject)=>{child.once('close',resolve);child.once('error',reject);});
 child.kill('SIGTERM');
 try{await Promise.race([closed,new Promise((_,reject)=>{timer=setTimeout(()=>{child.kill('SIGKILL');reject(Error('Owned protocol server did not close within 10 seconds'));},10000);})]);}
 finally{clearTimeout(timer);}
}
module.exports={stopChild};
