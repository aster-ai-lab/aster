use super::*;

#[test]
fn nvrtc_compiles_exact_xq_control_without_a_gpu() {
    let ptx = ptx().unwrap();
    println!(
        "XQ NVRTC abi={XQ_CUDA_ABI} source={} ptx={} bytes={}",
        source_sha256(),
        format!("{:x}", Sha256::digest(ptx.as_bytes())),
        ptx.len()
    );
}
