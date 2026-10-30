use std::path::Path;

fn scan(path: &Path, secrets: &[Vec<u8>], files: &mut usize) {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            scan(&entry.path(), secrets, files);
        } else {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(!secrets.iter().any(|secret| bytes.windows(secret.len()).any(|v| v == secret)),
                    "unredacted provider value in {}", entry.path().display());
            *files += 1;
        }
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "pass the original ACL and evidence directory");
    let config = a3s_code_core::CodeConfig::from_file(Path::new(&args[1])).unwrap();
    let mut values = Vec::new();
    for provider in &config.providers {
        values.extend(provider.api_key.iter());
        values.extend(provider.base_url.iter());
        values.extend(provider.headers.values());
        for model in &provider.models {
            values.extend(model.api_key.iter());
            values.extend(model.base_url.iter());
            values.extend(model.headers.values());
        }
    }
    let secrets: Vec<_> = values.iter().filter(|v| v.len() >= 6).map(|v| v.as_bytes().to_vec()).collect();
    let mut files = 0;
    assert!(!secrets.is_empty(), "no resolved provider values to scan");
    scan(Path::new(&args[2]), &secrets, &mut files);
    println!("provider_secret_scan=pass files={files}");
}
