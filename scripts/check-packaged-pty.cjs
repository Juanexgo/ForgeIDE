// Run with ELECTRON_RUN_AS_NODE=1 <ForgeIDE binary> scripts/check-packaged-pty.cjs <app root>.
const fs = require('node:fs'); const path = require('node:path');
const app = process.argv[2] && path.resolve(process.argv[2]);
if (!app) throw new Error('Pass the packaged .app path');
const resources = path.join(app, 'Contents/Resources');
const helper = path.join(resources, 'app.asar.unpacked/lib/prebuilds', process.platform + '-' + process.arch, 'spawn-helper');
fs.accessSync(helper, fs.constants.X_OK);
fs.accessSync(path.join(resources,'app.asar.unpacked/lib/backend/native/rg'),fs.constants.X_OK);
const pty = require(path.join(resources,'app.asar/node_modules/node-pty'));
const term = pty.spawn('/bin/sh',['-c','printf FORGE_PTY_OK'],{cols:80,rows:24,cwd:require('node:os').tmpdir(),env:{PATH:'/usr/bin:/bin',TERM:'xterm-256color'}});
let output = ''; const timer=setTimeout(()=>{term.kill();console.error('PTY timeout');process.exit(1);},5000);
term.onData(chunk=>output+=chunk);
term.onExit(e=>{clearTimeout(timer);if(e.exitCode!==0 || !output.includes('FORGE_PTY_OK')){console.error('PTY smoke failed');process.exitCode=1;}else console.log('Packaged PTY: PASS');});
