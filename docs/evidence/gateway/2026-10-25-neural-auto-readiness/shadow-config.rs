use a3s_acl::{Block,Value}; use std::io::{self,Read};
fn main(){
 let mut text=String::new();io::stdin().take(1048577).read_to_string(&mut text).unwrap();
 let mut doc=a3s_acl::parse(&text).unwrap();
 for b in &mut doc.blocks {
  if b.name=="entrypoints" && b.labels==["web"] {b.attributes.insert("address".into(),Value::String("127.0.0.1:18080".into()));}
  if b.name=="management" {b.attributes.insert("address".into(),Value::String("127.0.0.1:19090".into()));}
  if b.name=="managed" {b.attributes.insert("state_file".into(),Value::String("/data/deepseek-v41-xq-1017/private-neural-auto-config/shadow-journal.json".into()));}
 }
 print!("{}",a3s_acl::generate(&doc));
}
