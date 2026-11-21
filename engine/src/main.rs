use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, fs::{self, File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}};
use unicode_normalization::UnicodeNormalization;
const M: usize = 1024 * 1024;
type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Change { path: String, before: Option<String>, after: Option<String>, #[serde(default)] mode: Option<u32> }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Group { schema: u32, id: String, title: String, created: String, status: String, changes: Vec<Change>, #[serde(default)] legacy: bool, #[serde(default)] directories: Vec<String>, #[serde(default)] temporaries: Vec<String>, #[serde(default)] recovery: Option<String> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request { version: u32, root: String, state: String, action: String, #[serde(default)] id: Option<String>, #[serde(default)] group: Option<Group> }
fn io<T>(r: std::io::Result<T>) -> Result<T> {r.map_err(|e|e.to_string())}
fn id(s: &str) -> Result<()> {
 if s.len()!=36 || !s.bytes().enumerate().all(|(i,b)|if [8,13,18,23].contains(&i){b==b'-'}else{b.is_ascii_hexdigit()}) {return Err("Invalid group ID".into());} Ok(())
}
fn validate_path(s: &str) -> Result<()> {
 if s.is_empty() || s.len()>2048 || s.contains(['\\','\0',':']) || s.starts_with('/') {return Err("Use a portable relative path".into());}
 for part in s.split('/') {
  let lower=part.to_lowercase(); let stem=lower.split('.').next().unwrap_or("");
  if part.is_empty() || part=="." || part==".." || part.ends_with(['.',' ']) || [".git",".forgeide"].contains(&lower.as_str()) || lower.starts_with(".env") || ["id_rsa","id_ed25519","credentials","secrets"].contains(&stem) || ["pem","key","p12","pfx"].iter().any(|e|lower.ends_with(&format!(".{e}"))) || ["con","prn","aux","nul","com1","com2","com3","com4","com5","com6","com7","com8","com9","lpt1","lpt2","lpt3","lpt4","lpt5","lpt6","lpt7","lpt8","lpt9"].contains(&stem) {return Err("Protected or invalid path".into());}
 } Ok(())
}
fn checked(root: &Path, s: &str) -> Result<PathBuf> {
 validate_path(s)?;let mut p=root.to_path_buf();
 let parts:Vec<_>=s.split('/').collect();
 for (i,part) in parts.iter().enumerate() {p.push(part);match fs::symlink_metadata(&p) {
  Ok(meta)=>{if meta.file_type().is_symlink() {return Err("Symlinks are excluded".into());}if i+1<parts.len() && !meta.is_dir(){return Err("A parent is not a directory".into());}},
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(e.to_string())
 }} Ok(p)
}
fn content(root:&Path,s:&str)->Result<Option<String>> {
 let p=checked(root,s)?;
 match fs::symlink_metadata(&p) {
  Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(e)=>Err(e.to_string()),
  Ok(meta)=>{if !meta.is_file() || meta.len()>M as u64{return Err("Only regular UTF-8 files up to 1 MiB are supported".into());}
   let f=io(File::open(p))?;let mut bytes=Vec::new();io(f.take((M+1) as u64).read_to_end(&mut bytes))?;
   if bytes.len()>M || bytes.contains(&0){return Err("Binary or oversized file".into());}
   String::from_utf8(bytes).map(Some).map_err(|_|"Invalid UTF-8".into())
  }
 }
}
fn sync_dir(p:&Path)->Result<()> {
 #[cfg(unix)] {io(File::open(p).and_then(|f|f.sync_all()))?;}
 #[cfg(not(unix))] {let _=p;}
 Ok(())
}
fn atomic(p:&Path, data:&[u8], mode:Option<u32>, create:bool)->Result<()> {
 let parent=p.parent().ok_or("Missing parent")?;
 let mut t=io(tempfile::NamedTempFile::new_in(parent))?;
 io(t.write_all(data))?;
 #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;io(t.as_file().set_permissions(fs::Permissions::from_mode(mode.unwrap_or(0o600))))?;}
 #[cfg(not(unix))] {let _=mode;}
 io(t.as_file().sync_all())?;
 if create {t.persist_noclobber(p).map_err(|e|e.to_string())?;} else {t.persist(p).map_err(|e|e.to_string())?;}
 sync_dir(parent)
}
fn group_file(state:&Path,gid:&str)->Result<PathBuf>{id(gid)?;Ok(state.join("groups").join(format!("{gid}.json")))}
fn save(state:&Path,g:&Group)->Result<()> {atomic(&group_file(state,&g.id)?,&serde_json::to_vec(g).map_err(|e|e.to_string())?,Some(0o600),false)}
fn load(state:&Path,gid:&str)->Result<Group> {
 let p=group_file(state,gid)?;let meta=io(fs::symlink_metadata(&p))?;
 if !meta.is_file() || meta.file_type().is_symlink() || meta.len()>(16*M) as u64{return Err("Invalid journal".into());}
 let mut text=Vec::new();io(io(File::open(p))?.take((16*M+1) as u64).read_to_end(&mut text))?;
 let g:Group=serde_json::from_slice(&text).map_err(|e|e.to_string())?;validate_group(&g)?;Ok(g)
}
fn validate_group(g:&Group)->Result<()> {
 id(&g.id)?;
 if g.schema!=1 || g.title.len()>240 || g.changes.is_empty() || g.changes.len()>32 || !["review","applying","applied","reverting","reverted","conflict"].contains(&g.status.as_str()){return Err("Unsupported or invalid group".into());}
 let mut seen=HashSet::new();let mut keys=Vec::new();let mut size=0;
 for c in &g.changes {
  validate_path(&c.path)?;if c.mode.is_some_and(|m|m>0o777){return Err("Invalid permissions".into());}let key=c.path.nfc().collect::<String>().to_lowercase();
  if !seen.insert(key.clone()){return Err("Duplicate or aliased file path".into());}keys.push(key);
  if c.before==c.after {return Err("An operation must change the file".into());}
  for s in [&c.before,&c.after].into_iter().flatten(){if s.len()>M || s.contains('\0'){return Err("Invalid or oversized text".into());}size+=s.len();}
 }
 if size>8*M{return Err("Group exceeds 8 MiB".into());}
 for a in &keys {for b in &keys {if b.starts_with(&format!("{a}/")){return Err("A file cannot be another operation's parent".into());}}}
 for d in &g.directories {validate_path(d)?;}
 for (i,t) in g.temporaries.iter().enumerate(){if i>=g.changes.len()||t!=&temporary(g,i){return Err("Invalid managed temporary path".into());}}
 Ok(())
}
fn mode(p:&Path)->Result<u32> {
 #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;Ok(io(fs::metadata(p))?.permissions().mode()&0o777)}
 #[cfg(not(unix))] {let _=p;Ok(0o644)}
}
fn temporary(g:&Group,i:usize)->String {let parent=Path::new(&g.changes[i].path).parent().unwrap_or(Path::new("")).to_string_lossy().replace('\\',"/");format!("{}{sep}.forge-txn-{}-{i}.tmp",parent,g.id,sep=if parent.is_empty(){""}else{"/"})}
fn cleanup_temps(root:&Path,g:&Group)->Result<()> {for name in &g.temporaries{let p=checked(root,name)?;if p.exists(){io(fs::remove_file(&p))?;sync_dir(p.parent().unwrap())?;}}Ok(())}
fn write_change(root:&Path,c:&Change,expected:&Option<String>,next:&Option<String>,temp:&str)->Result<()> {
 if &content(root,&c.path)?!=expected{return Err(format!("Conflict: {} changed during transaction",c.path));}
 let p=checked(root,&c.path)?;
 match next {Some(s)=>{
  let tmp=checked(root,temp)?;let mut options=OpenOptions::new();options.write(true).create_new(true);
  #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
  let mut f=io(options.open(&tmp))?;io(f.write_all(s.as_bytes()))?;
  #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;io(f.set_permissions(fs::Permissions::from_mode(c.mode.unwrap_or(0o644))))?;}
  io(f.sync_all())?;drop(f);
  #[cfg(feature="fault-injection")] {if std::env::var("FORGE_ENGINE_TEST_PAUSE_STAGE").ok().as_deref()==Some("1"){std::thread::sleep(std::time::Duration::from_secs(60));}}
  if &content(root,&c.path)?!=expected{return Err("File changed before atomic replacement".into());}
  if expected.is_none(){io(fs::hard_link(&tmp,&p))?;io(fs::remove_file(&tmp))?;}else{io(fs::rename(&tmp,&p))?;}
  sync_dir(p.parent().unwrap())
 },None=>{io(fs::remove_file(&p))?;sync_dir(p.parent().unwrap())}}
}
fn cleanup_dirs(root:&Path,g:&Group) {
 let mut dirs=g.directories.clone();dirs.sort_by_key(|d|std::cmp::Reverse(d.len()));
 for d in dirs {if let Ok(p)=checked(root,&d){let _=fs::remove_dir(&p);}}
}
fn recover_group(root:&Path,state:&Path,g:&mut Group)->Result<()> {
 if g.status!="applying" && g.status!="reverting" {return Ok(());}
 let applying=g.status=="applying";
 if g.temporaries.is_empty(){g.temporaries=(0..g.changes.len()).map(|i|temporary(g,i)).collect();save(state,g)?;}
 cleanup_temps(root,g)?;
 let mut current=Vec::new();
 for c in &g.changes {match content(root,&c.path){Ok(v)=>current.push(v),Err(e)=>{g.status="conflict".into();g.recovery=Some(e);save(state,g)?;return Ok(());}}}
 if g.changes.iter().zip(&current).all(|(c,v)|v==if applying{&c.after}else{&c.before}) {
  g.status=if applying{"applied"}else{"reverted"}.into();g.recovery=Some("Recovered a fully written transaction".into());
 } else {
  if g.changes.iter().zip(&current).any(|(c,v)|v!=&c.before && v!=&c.after){g.status="conflict".into();g.recovery=Some("Interrupted operation diverged; no unknown content was overwritten".into());save(state,g)?;return Ok(());}
  for (c,v) in g.changes.iter().zip(&current){let original=if applying{&c.before}else{&c.after};if v!=original&&original.is_some()&&!checked(root,&c.path)?.parent().unwrap().is_dir(){g.status="conflict".into();g.recovery=Some("A parent directory changed; recovery left files intact".into());save(state,g)?;return Ok(());}}
  for (i,(c,v)) in g.changes.iter().zip(&current).enumerate().rev() {let original=if applying{&c.before}else{&c.after};if v!=original{write_change(root,c,v,original,&g.temporaries[i])?;}}
  g.status=if applying{"review"}else{"applied"}.into();g.recovery=Some("Rolled back an interrupted partial transaction".into());
 }
 if g.status=="review" || g.status=="reverted" {cleanup_dirs(root,g);}cleanup_temps(root,g)?;g.temporaries.clear();save(state,g)
}
fn list_ids(state:&Path)->Result<Vec<String>> {
 let mut out=Vec::new();for e in io(fs::read_dir(state.join("groups")))? {
  let e=io(e)?;let n=e.file_name().to_string_lossy().into_owned();
  if let Some(s)=n.strip_suffix(".json"){id(s)?;out.push(s.to_string());if out.len()>200{return Err("Archive history before exceeding 200 groups".into());}}
 }out.sort();Ok(out)
}
fn recover_all(root:&Path,state:&Path)->Result<()> {for gid in list_ids(state)?{let mut g=load(state,&gid)?;recover_group(root,state,&mut g)?;}Ok(())}
fn perform(root:&Path,state:&Path,g:&mut Group,revert:bool,mut hook:impl FnMut(usize)->Result<()>)->Result<()> {
 if g.status!=if revert{"applied"}else{"review"}{return Err("Group is not in the expected state".into());}
 for c in &mut g.changes {
  let expected=if revert{&c.after}else{&c.before};
  if &content(root,&c.path)?!=expected{return Err(format!("Conflict: {} changed since review",c.path));}
  if !revert {c.mode=Some(if c.before.is_some(){mode(&checked(root,&c.path)?)?}else{0o644});}
 }
 if !revert {
  let mut dirs=HashSet::new();
  for c in &g.changes {if c.after.is_some(){let mut p=Path::new(&c.path).parent();while let Some(d)=p{if d.as_os_str().is_empty(){break;}let s=d.to_string_lossy().replace('\\',"/");let full=checked(root,&s)?;if !full.exists(){dirs.insert(s);}p=d.parent();}}}
  g.directories=dirs.into_iter().collect();g.directories.sort_by_key(|d|d.len());
 }
 g.temporaries=(0..g.changes.len()).map(|i|temporary(g,i)).collect();for t in &g.temporaries{if checked(root,t)?.exists(){return Err("Managed temporary path already exists".into());}}
 g.status=if revert{"reverting"}else{"applying"}.into();g.recovery=None;save(state,g)?;
 let result:Result<()>=(||{
  for d in &g.directories {if !revert{let p=checked(root,d)?;if !p.exists(){io(fs::create_dir(&p))?;sync_dir(p.parent().unwrap())?;}}}
  for (i,c) in g.changes.iter().enumerate() {let (from,to)=if revert{(&c.after,&c.before)}else{(&c.before,&c.after)};write_change(root,c,from,to,&g.temporaries[i])?;hook(i)?;}
  Ok(())
 })();
 if let Err(e)=result {
  recover_group(root,state,g)?;
  return Err(format!("{e}; recovery state: {}",g.status));
 }
 g.status=if revert{"reverted"}else{"applied"}.into();if revert{cleanup_dirs(root,g);}cleanup_temps(root,g)?;g.temporaries.clear();save(state,g)
}
fn run(req:Request)->Result<Value> {
 if req.version!=1{return Err("Unsupported protocol version".into());}
 let root=io(fs::canonicalize(req.root))?;if !root.is_dir(){return Err("Workspace is not a directory".into());}
 io(fs::create_dir_all(Path::new(&req.state).join("groups")))?;
 let state=io(fs::canonicalize(req.state))?;
 let mut opts=OpenOptions::new();opts.read(true).write(true).create(true);
 #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;opts.mode(0o600);}
 let lock=io(opts.open(state.join(".forge-engine.lock")))?;
 lock.try_lock_exclusive().map_err(|_|"Another transaction is active in this workspace".to_string())?;
 // Kernel lock is released on normal exit, timeout and process death.
 recover_all(&root,&state)?;
 let result=match req.action.as_str() {
  "create"|"draft"=>{let mut g=req.group.ok_or("Group required")?;if req.action=="draft"{g.status="review".into();g.legacy=false;g.directories.clear();g.recovery=None;for c in &mut g.changes{c.before=content(&root,&c.path)?;c.mode=None;}}validate_group(&g)?;
   if g.status!="review" && !(g.legacy && g.status=="applied"){return Err("New groups must be review-only".into());}
   if list_ids(&state)?.len()>=200{return Err("History limit reached".into());}
   if group_file(&state,&g.id)?.exists(){return Err("Group already exists".into());}
   for c in &g.changes{if content(&root,&c.path)?!=if g.status=="applied"{c.after.clone()}else{c.before.clone()}{return Err(format!("Conflict before review: {}",c.path));}}
   save(&state,&g)?;serde_json::to_value(g).map_err(|e|e.to_string())?
  },
  "get"=>serde_json::to_value(load(&state,req.id.as_deref().ok_or("ID required")?)?).map_err(|e|e.to_string())?,
  "list"|"recover"=>{let mut out=Vec::new();for gid in list_ids(&state)?{let g=load(&state,&gid)?;out.push(json!({"id":g.id,"title":g.title,"created":g.created,"status":g.status,"legacy":g.legacy,"recovery":g.recovery,"files":g.changes.iter().map(|c|json!({"path":c.path,"kind":if c.before.is_none(){"create"}else if c.after.is_none(){"delete"}else{"edit"}})).collect::<Vec<_>>()}));}out.sort_by(|a,b|b["created"].as_str().cmp(&a["created"].as_str()));json!(out)},
  "apply"|"revert"=>{let mut g=load(&state,req.id.as_deref().ok_or("ID required")?)?;
   perform(&root,&state,&mut g,req.action=="revert",|_i|{
    #[cfg(feature="fault-injection")] {if std::env::var("FORGE_ENGINE_TEST_PAUSE_AFTER").ok().and_then(|s|s.parse::<usize>().ok())==Some(_i){std::thread::sleep(std::time::Duration::from_secs(60));}}
    Ok(())
   })?;serde_json::to_value(g).map_err(|e|e.to_string())?
  }, _=>return Err("Unknown operation".into())
 };
 Ok(result)
}
fn main() {
 let result=(||{let mut input=Vec::new();io(std::io::stdin().take((16*M+1) as u64).read_to_end(&mut input))?;if input.len()>16*M{return Err("Request exceeds 16 MiB".into());}run(serde_json::from_slice(&input).map_err(|e|e.to_string())?)})();
 let output=match result {Ok(v)=>json!({"ok":true,"result":v}),Err(e)=>json!({"ok":false,"error":{"code":"TRANSACTION_REJECTED","message":e}})};
 println!("{}",output);
}
