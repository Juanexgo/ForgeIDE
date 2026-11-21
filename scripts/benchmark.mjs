import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {projectGraph} from '../packages/workbench/lib/node/project-graph.js';
const root=await fs.mkdtemp(path.join(os.tmpdir(),'forge-benchmark-'));
const results=[];
try {
 for(const count of [100,1000,10000]) {
  const dir=path.join(root,String(count));await fs.mkdir(dir);
  for(let i=0;i<count;i++)await fs.writeFile(path.join(dir,'m'+i+'.ts'),(i+1<count?"import './m"+(i+1)+".js';\n":'')+'export const value='+i+';\n');
  const times=[];let last;
  for(let run=0;run<3;run++){const start=performance.now();last=await projectGraph(dir);times.push(performance.now()-start);}
  times.sort((a,b)=>a-b);
  results.push({fixtureFiles:count,resolvedModules:last.nodes.length,edges:last.edges.length,truncated:last.truncated,samples:times,p50:times[1],p95:times[2],rssMB:Math.round(process.memoryUsage().rss/1048576)});
 }
 const report={date:new Date().toISOString(),node:process.version,platform:process.platform,arch:process.arch,cpu:os.cpus()[0]?.model,ramGB:Math.round(os.totalmem()/1073741824),scope:'Static JS/TS graph only; generated fixtures; 3 warm-process samples. No editor/input/IDE startup benchmark or competitor comparison.',results};
 const output=process.argv[2]||'benchmarks/graph-local.json';await fs.writeFile(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
} finally {await fs.rm(root,{recursive:true,force:true});}
