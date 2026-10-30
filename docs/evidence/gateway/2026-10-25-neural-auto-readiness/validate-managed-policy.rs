use std::io::{self,Read};
fn main(){
 let mut text=String::new();io::stdin().take(1048577).read_to_string(&mut text).unwrap();assert!(text.len()<=1048576);
 let mut config=a3s_gateway::config::GatewayConfig::from_acl(&text).unwrap();
 let bootstrap=a3s_gateway::config::GatewayConfig::from_acl(&std::fs::read_to_string("/etc/a3s/gateway-managed.acl").unwrap()).unwrap();
 config.managed.static_objects=bootstrap.managed.static_objects;
 config.validate().unwrap();
 let inference=config.inference.as_ref().unwrap();
 let auto=inference.routes.values().find_map(|route|route.models.get("auto")).unwrap();
 println!("snapshot configuration valid; auto targets={}, routing={:?}",auto.targets.len(),auto.routing.as_ref().map(|p|p.decision_protocol));
}
