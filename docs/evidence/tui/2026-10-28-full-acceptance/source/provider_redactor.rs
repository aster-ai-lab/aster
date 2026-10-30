use std::io::{Read, Write};
use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 2);
    let config = a3s_code_core::CodeConfig::from_file(Path::new(&args[1])).unwrap();
    let mut secrets = Vec::new();
    for provider in &config.providers {
        secrets.extend(provider.api_key.iter());
        secrets.extend(provider.base_url.iter());
        secrets.extend(provider.headers.values());
        for model in &provider.models {
            secrets.extend(model.api_key.iter());
            secrets.extend(model.base_url.iter());
            secrets.extend(model.headers.values());
        }
    }
    let mut input = String::new();
    std::io::stdin().take(1_048_577).read_to_string(&mut input).unwrap();
    assert!(input.len() <= 1_048_576);
    for value in secrets.into_iter().filter(|value| value.len() >= 6) {
        input = input.replace(value, "[redacted]");
    }
    std::io::stdout().write_all(input.as_bytes()).unwrap();
}
