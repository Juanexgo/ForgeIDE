import * as fs from 'node:fs';import * as path from 'node:path';import * as os from 'node:os';
export function findTool(name:string):string|undefined {
 if(!/^[a-zA-Z0-9._-]+$/.test(name))throw new Error('Invalid executable name');
 const dirs=(process.env.PATH||'').split(path.delimiter).filter(Boolean);
 if(process.platform==='darwin')dirs.push('/opt/homebrew/bin','/usr/local/bin','/Applications/Docker.app/Contents/Resources/bin');
 if(process.platform==='win32'){const pf=process.env.ProgramFiles||'C:\\Program Files';dirs.push(path.join(pf,'GitHub CLI'),path.join(pf,'Git/cmd'),path.join(pf,'Docker/Docker/resources/bin'));}
 dirs.push(path.join(os.homedir(),'.cargo/bin'),path.join(os.homedir(),'go/bin'));
 for(const dir of [...new Set(dirs)])for(const ext of process.platform==='win32'?['.exe','.cmd','']:['']){const file=path.join(dir,name+ext);try{fs.accessSync(file,process.platform==='win32'?fs.constants.F_OK:fs.constants.X_OK);if(fs.statSync(file).isFile())return file;}catch{}}
 return undefined;
}
export function languageTools():{language:string;tools:{name:string;path:string|null}[];extension:string}[]{
 const profiles=[{language:'TypeScript / JavaScript',tools:['node'],extension:'builtin: vscode.typescript-language-features'},{language:'Python',tools:['python3','pyright-langserver'],extension:'ms-pyright.pyright'},{language:'Rust',tools:['cargo','rust-analyzer'],extension:'rust-lang.rust-analyzer'},{language:'Go',tools:['go','gopls'],extension:'golang.go'},{language:'Java',tools:['java','javac'],extension:'redhat.java'},{language:'C / C++',tools:['clangd'],extension:'llvm-vs-code-extensions.vscode-clangd'},{language:'Swift',tools:['swift','sourcekit-lsp'],extension:'swiftlang.swift-vscode'}];
 return profiles.map(p=>({language:p.language,extension:p.extension,tools:p.tools.map(name=>({name,path:findTool(name)||null}))}));
}
