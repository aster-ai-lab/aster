//! Read-only audit of a pinned public tokenizer. No inference or GPU allocation.
use aster_deepseek::tokenizer::{CompressedVocabulary, MAX_TOKENIZER_BYTES};
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("expected one public tokenizer path")?;
    if args.next().is_some() {
        return Err("expected one public tokenizer path".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((MAX_TOKENIZER_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let vocabulary = CompressedVocabulary::from_official_tokenizer(&bytes)?;
    println!(
        "{}",
        serde_json::json!({"raw_vocabulary":vocabulary.raw_vocabulary_size(),"compressed_map_u32le_sha256":vocabulary.mapping_sha256(),"compressed_pad":vocabulary.compress(2)?,"gpu_executed":false})
    );
    Ok(())
}
