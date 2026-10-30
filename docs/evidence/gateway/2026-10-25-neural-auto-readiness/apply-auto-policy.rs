use a3s_acl::Block;
use std::io::{self,Read};
fn patch(blocks:&mut [Block], model:&Block)->usize{
 let mut count=0;
 for b in blocks{
  if b.name=="models" && b.labels==["auto"] { *b=model.clone();count+=1; }
  else {count+=patch(&mut b.blocks,model);}
 }
 count
}
fn main(){
 let path=std::env::args().nth(1).unwrap();
 let policy=a3s_acl::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
 let service=policy.blocks.iter().find(|b|b.name=="services").unwrap();
 let model=policy.blocks.iter().find(|b|b.name=="models").unwrap();
 let mut text=String::new();io::stdin().take(1048577).read_to_string(&mut text).unwrap();assert!(text.len()<=1048576);
 let mut doc=a3s_acl::parse(&text).unwrap();
 assert!(!doc.blocks.iter().any(|b|b.name=="services"&&b.labels==service.labels),"pool already declared");
 let count=patch(&mut doc.blocks,model);
 assert!(count<=1,"at most one auto model must exist");
 if count==0 {
  let inference=doc.blocks.iter_mut().find(|b|b.name=="inference").unwrap();
  let routes=inference.blocks.iter_mut().filter(|b|b.name=="routes").collect::<Vec<_>>();
  assert_eq!(routes.len(),1,"one explicit inference route must exist");
  routes.into_iter().next().unwrap().blocks.insert(0,model.clone());
 }
 doc.blocks.insert(0,service.clone());
 let rendered=a3s_acl::generate(&doc);
 let reparsed=a3s_acl::parse(&rendered).unwrap();
 assert_eq!(a3s_acl::canonical_bytes(&doc).unwrap(),a3s_acl::canonical_bytes(&reparsed).unwrap());
 print!("{rendered}");
}
