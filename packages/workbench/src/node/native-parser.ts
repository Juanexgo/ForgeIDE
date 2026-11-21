import {spawn} from 'node:child_process';
import {existsSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import * as path from 'node:path';
export interface ImportRecord {kind:string;spec:string;names:string[];level:number;scope:string[];}
export interface ParseInput {path:string;language:string;source:string;}
export interface ParseResult {path:string;imports:ImportRecord[];errors:boolean;meta?:{name?:string;lib?:string;dependencies?:Record<string,string>};error?:string;}
export function parserPath():string {
 const exe='forge-parser'+(process.platform==='win32'?'.exe':'');const resources=(process as NodeJS.Process&{resourcesPath?:string}).resourcesPath;
 const choices=[process.env.FORGE_PARSER_PATH,resources&&path.join(resources,'engine',exe),fileURLToPath(new URL('../../../engine/'+exe,import.meta.url)),fileURLToPath(new URL('../../../../engine/bin/'+exe,import.meta.url))];
 const found=choices.find((p):p is string=>!!p&&existsSync(p));if(!found)throw Error('Forge parser is missing; rebuild the Rust helpers or reinstall the application');return found;
}
export function parseImports(files:ParseInput[]):Promise<ParseResult[]> {
 const input=JSON.stringify({version:1,files});if(Buffer.byteLength(input)>32*1024*1024)throw Error('Parser request exceeds 32 MiB');
 return new Promise((resolve,reject)=>{const child=spawn(parserPath(),[],{stdio:['pipe','pipe','pipe'],windowsHide:true});const chunks:Buffer[]=[];let bytes=0,done=false;const timer=setTimeout(()=>{child.kill('SIGKILL');finish(Error('Import parser exceeded 10 seconds'));},10000);
 const finish=(error?:Error,value?:ParseResult[]):void=>{if(done)return;done=true;clearTimeout(timer);error?reject(error):resolve(value!);};
 child.stdout.on('data',(b:Buffer)=>{bytes+=b.length;if(bytes>8*1024*1024){child.kill('SIGKILL');finish(Error('Parser result exceeds 8 MiB'));}else chunks.push(b);});child.stderr.on('data',()=>{});child.on('error',e=>finish(e));child.stdin.on('error',()=>{});
 child.on('close',code=>{if(done)return;try{const j=JSON.parse(Buffer.concat(chunks).toString('utf8'));if(code!==0||!j.ok||!Array.isArray(j.files))throw Error(j.error||'Parser process failed');finish(undefined,j.files);}catch(e){finish(e instanceof Error?e:Error(String(e)));}});child.stdin.end(input);
 });
}
