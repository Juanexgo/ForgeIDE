import {spawnSync} from 'node:child_process';
const audit=spawnSync(process.platform==='win32'?'yarn.cmd':'yarn',['audit','--json'],{encoding:'utf8',maxBuffer:16*1024*1024,shell:process.platform==='win32'});
if(audit.error)throw audit.error;
const rows=(audit.stdout||'').split('\n').filter(x=>x.startsWith('{')).map(x=>JSON.parse(x));const summary=rows.find(x=>x.type==='auditSummary');
if(!summary)throw Error('No completed audit summary; registry failure must not pass');
console.log(JSON.stringify(summary.data));if(Object.values(summary.data.vulnerabilities).some(n=>n>0))process.exit(1);
