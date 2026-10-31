//! Operator diagnostic for an already copied GGUF header; never runs inference.
use std::collections::BTreeMap;
use std::io::Read;

use aster_deepseek::weights::xq::gguf::{XqGgufHeader, XQ_HEADER_BYTES, XQ_HEADER_SHA256};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let path = arguments
        .next()
        .ok_or("expected exactly one copied header path")?;
    if arguments.next().is_some() {
        return Err("expected exactly one copied header path".into());
    }
    let mut header = Vec::new();
    std::fs::File::open(path)?
        .take(XQ_HEADER_BYTES as u64 + 1)
        .read_to_end(&mut header)?;
    let parsed = XqGgufHeader::from_pinned_bytes(&header)?;
    let mut totals = BTreeMap::<String, (u64, u64)>::new();
    let mut catalog = Sha256::new();
    for (name, tensor) in parsed.tensors() {
        let total = totals.entry(tensor.kind.gguf_id().to_string()).or_default();
        total.0 += 1;
        total.1 += tensor.bytes;
        catalog.update(format!(
            "{}:{}:{:?}:{}:{}\n",
            name,
            tensor.kind.gguf_id(),
            tensor.shape,
            tensor.absolute_offset,
            tensor.bytes
        ));
    }
    println!(
        "{}",
        serde_json::json!({
            "header_sha256": XQ_HEADER_SHA256, "header_bytes": header.len(),
            "tensor_count": parsed.tensors().len(), "metadata_count": parsed.metadata().len(),
            "alignment": parsed.alignment(), "data_offset": parsed.data_offset(),
            "type_totals": totals, "catalog_sha256": format!("{:x}", catalog.finalize()),
            "tensor_payload_integrity_verified": false, "gpu_inference_executed": false,
        })
    );
    Ok(())
}
