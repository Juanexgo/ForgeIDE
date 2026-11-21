import {spawnSync} from 'node:child_process';
import fs from 'node:fs';import path from 'node:path';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('..',import.meta.url));
const cargo=process.env.FORGE_CARGO || 'cargo';const toolchain=process.env.FORGE_RUST_TOOLCHAIN;
if(toolchain && !/^[a-zA-Z0-9_.-]+$/.test(toolchain))throw Error('Invalid toolchain');
const args=[...(toolchain?['+'+toolchain]:[]),'build','--release','--locked','--manifest-path',path.join(root,'engine/Cargo.toml')];
const result=spawnSync(cargo,args,{stdio:'inherit',env:process.env});if(result.error)throw result.error;if(result.status!==0)process.exit(result.status||1);
const target=process.env.CARGO_TARGET_DIR||path.join(root,'engine/target');
fs.mkdirSync(path.join(root,'engine/bin'),{recursive:true});
for(const name of ['forge-engine','forge-parser']){const exe=name+(process.platform==='win32'?'.exe':'');fs.copyFileSync(path.join(target,'release',exe),path.join(root,'engine/bin',exe));if(process.platform!=='win32')fs.chmodSync(path.join(root,'engine/bin',exe),0o755);}
