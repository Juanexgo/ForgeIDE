import * as fs from 'node:fs';
import * as path from 'node:path';
import { createHash, randomUUID } from 'node:crypto';
import type { Proposal } from '../common/forge-protocol.js';
const hash = (s: string): string => createHash('sha256').update(s).digest('hex');
const MAX = 1024 * 1024;
export class SafeStore {
 readonly root: string;
 readonly dir: string;
 constructor(root: string, state: string) {
  this.root = fs.realpathSync(root);
  if (!fs.statSync(this.root).isDirectory()) throw new Error('Open a directory');
  this.dir = path.join(state, hash(this.root));
  fs.mkdirSync(this.dir, {recursive: true, mode: 0o700});
  // A durable intent is written before replacing a file. Reconcile a process crash.
  for (const p of this.list()) {
   if (p.status === 'applying' || p.status === 'reverting') {
    const actual = this.read(p.path);
    const applying = p.status === 'applying';
    p.status = actual === (applying ? p.after : p.before) ? (applying ? 'applied' : 'reverted')
      : actual === (applying ? p.before : p.after) ? (applying ? 'review' : 'applied') : 'conflict';
    this.save(p);
   }
  }
 }
 private resolve(relative: string): string {
  if (typeof relative !== 'string' || !relative || path.isAbsolute(relative) || relative.includes('\\') || relative.includes('\0')) throw new Error('Use a relative file path');
  const parts = relative.split('/');
  if (parts.some(x => !x || x === '.' || x === '..' || x === '.git' || x === '.forgeide' || x.startsWith('.env') || /^(id_rsa|id_ed25519)$|\.(pem|key)$/i.test(x))) throw new Error('Protected or invalid path');
  const file = path.join(this.root, ...parts);
  let current = this.root;
  for (const part of parts) {
   current = path.join(current, part);
   if (fs.lstatSync(current).isSymbolicLink()) throw new Error('Symlinks are excluded');
  }
  if (!fs.statSync(file).isFile()) throw new Error('Only existing regular files are supported');
  return file;
 }
 read(relative: string): string {
  const file = this.resolve(relative);
  const fd = fs.openSync(file, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0));
  try {
   if (fs.fstatSync(fd).size > MAX) throw new Error('File exceeds 1 MiB');
   const buffer = Buffer.alloc(MAX + 1);
   let count = 0, n = 0;
   do { n = fs.readSync(fd, buffer, count, buffer.length - count, null); count += n; } while(n && count < buffer.length);
   if(count > MAX) throw new Error('File exceeds 1 MiB');
   const bytes = buffer.subarray(0, count);
   if (bytes.includes(0)) throw new Error('Binary files are excluded');
   return new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(bytes);
  } finally { fs.closeSync(fd); }
 }
 private filename(id: string): string {
  if (!/^[0-9a-f-]{36}$/.test(id)) throw new Error('Invalid proposal ID');
  return path.join(this.dir, id + '.json');
 }
 private save(p: Proposal): void {
  const dest = this.filename(p.id), temp = dest + '.' + randomUUID() + '.tmp';
  const fd = fs.openSync(temp, 'wx', 0o600);
  try {fs.writeFileSync(fd, JSON.stringify(p)); fs.fsyncSync(fd);} finally {fs.closeSync(fd);}
  fs.renameSync(temp, dest);
 }
 list(): Proposal[] {
  return fs.readdirSync(this.dir).filter(n => /^[0-9a-f-]{36}\.json$/.test(n)).map(n => JSON.parse(fs.readFileSync(path.join(this.dir, n), 'utf8')) as Proposal).sort((a,b) => b.created.localeCompare(a.created));
 }
 propose(relative: string, after: string): Proposal {
  if (typeof after !== 'string' || Buffer.byteLength(after) > MAX || after.includes('\0') || /[\uD800-\uDFFF]/u.test(after)) throw new Error('Replacement must be UTF-8 text under 1 MiB');
  const p: Proposal = {id: randomUUID(), path: relative, before: this.read(relative), after, status: 'review', created: new Date().toISOString(), verification: 'not_run'};
  this.save(p); return p;
 }
 private change(id: string, revert: boolean): Proposal {
  const lock = path.join(this.dir, 'operation.lock');
  let fd: number;
  try {fd = fs.openSync(lock, 'wx', 0o600);} catch {throw new Error('Another operation is active, or a previous process stopped. Close ForgeIDE and inspect operation.lock before retrying.');}
  try {
   const p = JSON.parse(fs.readFileSync(this.filename(id), 'utf8')) as Proposal;
   if (p.status !== (revert ? 'applied' : 'review')) throw new Error('Proposal is not in the expected state');
   const expected = revert ? p.after : p.before, replacement = revert ? p.before : p.after;
   if (this.read(p.path) !== expected) throw new Error('Conflict: file changed since review. No content was overwritten.');
   const file = this.resolve(p.path), temp = path.join(path.dirname(file), '.forge-' + randomUUID() + '.tmp');
   const mode = fs.statSync(file).mode;
   const out = fs.openSync(temp, 'wx', mode & 0o777);
   try {fs.writeFileSync(out, replacement); fs.fsyncSync(out);} finally {fs.closeSync(out);}
   try {
    p.status = revert ? 'reverting' : 'applying'; this.save(p);
    if (this.read(p.path) !== expected) { p.status = revert ? 'applied' : 'review'; this.save(p); throw new Error('Conflict during write'); }
    fs.renameSync(temp, file);
    p.status = revert ? 'reverted' : 'applied'; this.save(p);
   } finally {if(fs.existsSync(temp)) fs.unlinkSync(temp);}
   return p;
  } finally {fs.closeSync(fd); fs.unlinkSync(lock);}
 }
 recordState(id:string,status:Proposal['status']):void {const p=JSON.parse(fs.readFileSync(this.filename(id),'utf8')) as Proposal;p.status=status;this.save(p);}
 apply(id: string): Proposal {return this.change(id, false);}
 revert(id: string): Proposal {return this.change(id, true);}
}
