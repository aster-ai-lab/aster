use a3s_acl::{Block,Value};
use std::io::{self,Read};
fn find<'a>(blocks:&'a [Block],kind:&str,label:&str)->Option<&'a Block>{
 for block in blocks {
  if block.name==kind && block.labels.first().is_some_and(|s|s==label){return Some(block);}
  if let Some(found)=find(&block.blocks,kind,label){return Some(found);}
 }
 None
}
fn main(){
 let mut text=String::new();io::stdin().take(1048577).read_to_string(&mut text).unwrap();assert!(text.len()<=1048576);
 let doc=a3s_acl::parse(&text).unwrap();
 let mut auto=find(&doc.blocks,"models","auto").unwrap().clone();
 let deep=find(&doc.blocks,"models","deepseek-v4.1-flash").unwrap();
 let prism=find(&doc.blocks,"models","prism-27b").unwrap();
 let mut pool=find(&doc.blocks,"services","power-gpu-0").unwrap().clone();
 pool.labels=vec!["auto-prism-pool".into()];
 let mut servers=Vec::new();
 for name in ["power-gpu-0","power-gpu-1","power-gpu-2","power-gpu-3","power-gpu-4","power-gpu-5"] {
  let service=find(&doc.blocks,"services",name).unwrap();
  let lb=service.blocks.iter().find(|b|b.name=="load_balancer").unwrap();
  match lb.attributes.get("servers").unwrap() {Value::List(values)=>servers.extend(values.clone()),_=>panic!("servers must be a list")}
 }
 let lb=pool.blocks.iter_mut().find(|b|b.name=="load_balancer").unwrap();
 lb.attributes.insert("servers".into(),Value::List(servers));
 lb.attributes.insert("strategy".into(),Value::String("least-connections".into()));
 let mut economy=prism.blocks.iter().find(|b|b.name=="targets").unwrap().clone();
 let mut primary=deep.blocks.iter().find(|b|b.name=="targets").unwrap().clone();
 let economy_id="fd5202fd-073b-5a51-a5e7-d2bb50633cf7";
 let primary_id="7290dd47-c92b-54be-8de3-f4c1560cd8c8";
 economy.labels=vec![economy_id.into()];economy.attributes.insert("service".into(),Value::String("auto-prism-pool".into()));
 primary.labels=vec![primary_id.into()];
 for target in [&mut economy,&mut primary]{
  target.attributes.insert("priority".into(),Value::Number(0.0));
  target.attributes.insert("weight".into(),Value::Number(1.0));
  assert!(target.attributes.keys().all(|k| ["service","upstream_model","priority","weight","api_key_env","protocol","translate"].contains(&k.as_str())));
 }
 let routing=a3s_acl::parse(&format!(r#"routing {{
 mode = "smart_saving"
 decision_model = "prism-27b"
 decision_protocol = "openai_chat"
 economy_targets = ["{economy_id}"]
 primary_targets = ["{primary_id}"]
 simple_probability_threshold_bp = 8000
 competence_floor_bp = 5000
 }}"#)).unwrap().blocks.remove(0);
 assert!(auto.attributes.keys().all(|k|["model_id","prefix_affinity","prefix_parallelism","latency_bias"].contains(&k.as_str())));
 auto.blocks=vec![economy,primary,routing];
 let policy=a3s_acl::Document{blocks:vec![pool,auto]};
 print!("{}",a3s_acl::generate(&policy));
}
