use a3s_acl::Block; use std::io::{self,Read};
fn show(b:&Block,depth:usize){
 println!("{} {} attrs={:?}", " ".repeat(depth), b.name,b.attributes.keys().collect::<Vec<_>>());
 for (key,value) in &b.attributes {if ["address","state_file","directory","path","cache_dir"].contains(&key.as_str()){println!("{} {key}={value:?}"," ".repeat(depth));}}
 for child in &b.blocks{show(child,depth+1);}
}
fn main(){let mut text=String::new();io::stdin().take(1048577).read_to_string(&mut text).unwrap();let doc=a3s_acl::parse(&text).unwrap();for b in &doc.blocks{show(b,0);}}