use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::io::{Read,Write};
use tree_sitter::{Node,Parser};
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Request{version:u32,files:Vec<Input>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Input{path:String,language:String,source:String}
#[derive(Serialize)]#[serde(rename_all="camelCase")]struct Parsed{path:String,imports:Vec<Import>,errors:bool,meta:Value}
#[derive(Serialize)]struct Import{kind:String,spec:String,names:Vec<String>,level:usize,scope:Vec<String>}
fn text<'a>(n:Node,s:&'a str)->&'a str{n.utf8_text(s.as_bytes()).unwrap_or("")}
fn import(kind:&str,spec:&str,names:Vec<String>,level:usize,scope:&[String])->Import{Import{kind:kind.into(),spec:spec.into(),names,level,scope:scope.into()}}
fn rust_use(n:Node,s:&str,prefix:&str,scope:&[String],out:&mut Vec<Import>,depth:usize){
 if depth>128||out.len()>4096{return;}
 match n.kind(){
  "use_as_clause"=>{if let Some(p)=n.child_by_field_name("path"){rust_use(p,s,prefix,scope,out,depth+1);}},
  "scoped_use_list"=>{let path=n.child_by_field_name("path").map(|v|text(v,s)).unwrap_or("");let base=if prefix.is_empty(){path.to_string()}else{format!("{prefix}::{path}")};if let Some(list)=n.child_by_field_name("list"){rust_use(list,s,&base,scope,out,depth+1);}},
  "use_list"=>{let mut c=n.walk();for ch in n.named_children(&mut c){rust_use(ch,s,prefix,scope,out,depth+1);}},
  _=>{let v=text(n,s);let spec=if prefix.is_empty(){v.to_string()}else{format!("{prefix}::{v}")};out.push(import("use",&spec,vec![],0,scope));}
 }
}
fn parse(f:Input)->Result<Parsed,String>{
 if f.source.len()>512*1024||f.path.len()>2048{return Err("Source exceeds parser limits".into());}
 if f.language=="cargo"{
  let m:toml::Value=toml::from_str(&f.source).map_err(|_|"Invalid Cargo.toml")?;let mut deps=serde_json::Map::new();
  for section in ["dependencies","dev-dependencies","build-dependencies"]{if let Some(table)=m.get(section).and_then(|v|v.as_table()){for(name,val)in table{if let Some(p)=val.get("path").and_then(|v|v.as_str()){deps.insert(name.replace('-',"_"),json!(p));}}}}
  return Ok(Parsed{path:f.path,imports:vec![],errors:false,meta:json!({"name":m.get("package").and_then(|v|v.get("name")).and_then(|v|v.as_str()),"lib":m.get("lib").and_then(|v|v.get("path")).and_then(|v|v.as_str()),"dependencies":deps})});
 }
 let lang=match f.language.as_str(){"python"=>tree_sitter_python::LANGUAGE.into(),"go"=>tree_sitter_go::LANGUAGE.into(),"rust"=>tree_sitter_rust::LANGUAGE.into(),_=>return Err("Unsupported parser language".into())};
 let mut p=Parser::new();p.set_language(&lang).map_err(|e|e.to_string())?;
 #[allow(deprecated)]p.set_timeout_micros(500_000);
 let tree=p.parse(&f.source,None).ok_or("Parser time budget exceeded")?;let mut errors=tree.root_node().has_error();let mut imports=vec![];let mut pending=vec![(tree.root_node(),Vec::<String>::new(),0usize)];let mut seen=0;
 while let Some((n,scope,depth))=pending.pop(){seen+=1;if seen>100000||depth>256||imports.len()>4096{errors=true;break;}
  if f.language=="python"&&(n.kind()=="import_statement"||n.kind()=="import_from_statement"){
   let module=n.child_by_field_name("module_name").map(|v|text(v,&f.source)).unwrap_or("");let level=module.chars().take_while(|c|*c=='.').count();let mut names=vec![];let mut c=n.walk();
   for ch in n.named_children(&mut c){if Some(ch)==n.child_by_field_name("module_name"){continue;}if ch.kind()=="dotted_name"||ch.kind()=="aliased_import"{let v=ch.child_by_field_name("name").unwrap_or(ch);names.push(text(v,&f.source).to_string());}}
   if n.kind()=="import_statement"{for name in names{imports.push(import("module",&name,vec![],0,&[]));}}else{imports.push(import("pyfrom",module.trim_start_matches('.'),names,level,&[]));}continue;
  }
  if f.language=="go"&&n.kind()=="import_spec"{if let Some(path)=n.child_by_field_name("path"){let v=text(path,&f.source);let spec=if v.starts_with('`'){v.trim_matches('`').to_string()}else{serde_json::from_str::<String>(v).unwrap_or_default()};imports.push(import("module",&spec,vec![],0,&[]));}continue;}
  if f.language=="rust"&&n.kind()=="use_declaration"{if let Some(arg)=n.child_by_field_name("argument"){rust_use(arg,&f.source,"",&scope,&mut imports,0);}continue;}
  if f.language=="rust"&&n.kind()=="mod_item"{let name=n.child_by_field_name("name").map(|v|text(v,&f.source)).unwrap_or("");if let Some(body)=n.child_by_field_name("body"){let mut next=scope.clone();next.push(name.into());pending.push((body,next,depth+1));}else{imports.push(import("mod",name,vec![],0,&scope));}continue;}
  let mut c=n.walk();for ch in n.named_children(&mut c){pending.push((ch,scope.clone(),depth+1));}
 }
 Ok(Parsed{path:f.path,imports,errors,meta:Value::Null})
}
fn run()->Result<Value,String>{let mut input=Vec::new();std::io::stdin().take(32*1024*1024+1).read_to_end(&mut input).map_err(|e|e.to_string())?;if input.len()>32*1024*1024{return Err("Parser frame exceeds 32 MiB".into());}let r:Request=serde_json::from_slice(&input).map_err(|e|e.to_string())?;if r.version!=1||r.files.len()>1128{return Err("Unsupported parser request".into());}let mut out=vec![];for f in r.files{let path=f.path.clone();match parse(f){Ok(p)=>out.push(serde_json::to_value(p).unwrap()),Err(e)=>out.push(json!({"path":path,"imports":[],"errors":true,"error":e,"meta":null}))}}Ok(json!({"ok":true,"files":out}))}
fn main(){let out=run().unwrap_or_else(|e|json!({"ok":false,"error":e}));let _=std::io::stdout().write_all(out.to_string().as_bytes());}
