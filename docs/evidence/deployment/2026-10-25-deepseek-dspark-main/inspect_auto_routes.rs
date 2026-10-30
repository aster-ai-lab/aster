use a3s_acl::{Block, Value};
use std::io::{self, Read};
const ALIAS: &str = "auto";
const SERVICE: &str = "platform/apofasi";
fn named(s: &str) -> bool {
    s == ALIAS || s == SERVICE
}
fn safe(k: &str) -> bool {
    matches!(
        k,
        "service"
            | "service_name"
            | "native_service"
            | "upstream_model"
            | "request_timeout"
            | "connect_timeout"
            | "stream_idle_timeout"
            | "stream_total_timeout" | "mode" | "decision_model" | "routing" | "model_id" | "alias" | "economy_targets" | "primary_targets" | "target_id" | "context_window"
    )
}
fn value(path: &str, v: &Value, relevant: bool, depth: usize) {
    if depth > 32 {
        return;
    }
    match v {
        Value::Object(entries) => {
            if relevant {
                println!(
                    "object {path}: keys={:?}",
                    entries.iter().map(|(k, _)| k).collect::<Vec<_>>()
                );
            }
            for (k, v) in entries {
                let child = format!("{path}.{k}");
                if relevant && safe(k) {
                    println!("{child}={v:?}");
                }
                value(&child, v, relevant || named(k), depth + 1);
            }
        }
        Value::List(values) => {
            for (i, v) in values.iter().enumerate() {
                value(&format!("{path}[{i}]"), v, relevant, depth + 1)
            }
        }
        _ => {}
    }
}
fn block(path: &str, b: &Block, parent: bool, depth: usize) {
    if depth > 32 {
        return;
    }
    let relevant = parent || b.labels.iter().any(|s| named(s));
    let path = format!(
        "{path}/{}{}",
        b.name,
        if b.labels.iter().any(|s| named(s)) {
            format!("{:?}", b.labels)
        } else {
            String::new()
        }
    );
    if relevant {
        println!(
            "block {path}: keys={:?}",
            b.attributes.keys().collect::<Vec<_>>()
        );
    }
    for (k, v) in &b.attributes {
        if relevant && safe(k) {
            println!("{path}.{k}={v:?}");
        }
        value(&format!("{path}.{k}"), v, relevant || named(k), depth + 1);
    }
    for c in &b.blocks {
        block(&path, c, relevant, depth + 1)
    }
}
fn main() {
    let mut input = String::new();
    io::stdin()
        .take(1048577)
        .read_to_string(&mut input)
        .unwrap();
    assert!(input.len() <= 1048576);
    let doc = a3s_acl::parse(&input).expect("ACL must parse");
    for b in &doc.blocks {
        block("", b, false, 0)
    }
}
