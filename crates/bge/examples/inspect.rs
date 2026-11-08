//! Prints the input/output tensor names of one ONNX model and, on dummy
//! input, every output's shape. Diagnostic for export verification.
use ort::value::TensorRef;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: inspect <model.onnx>");
    let mut session = ort::session::Session::builder()
        .expect("builder")
        .commit_from_file(&path)
        .expect("session");
    println!("inputs:");
    for input in &session.inputs {
        println!("  {}", input.name);
    }
    println!("outputs:");
    for output in &session.outputs {
        println!("  {}", output.name);
    }
    let has_token_types = session
        .inputs
        .iter()
        .any(|input| input.name == "token_type_ids");
    let first_output = session
        .outputs
        .first()
        .map(|output| output.name.clone())
        .unwrap_or_default();

    let rows = 1_usize;
    let cols = 8_usize;
    let ids = vec![0_i64; rows * cols];
    let mask = vec![1_i64; rows * cols];
    let id_tensor = TensorRef::from_array_view(([rows, cols], ids.as_slice())).expect("ids");
    let mask_tensor = TensorRef::from_array_view(([rows, cols], mask.as_slice())).expect("mask");
    let outputs = if has_token_types {
        let token_type_values = vec![0_i64; rows * cols];
        let token_types = TensorRef::from_array_view(([rows, cols], token_type_values.as_slice()))
            .expect("token types");
        session
            .run(ort::inputs![
                "input_ids" => id_tensor,
                "attention_mask" => mask_tensor,
                "token_type_ids" => token_types
            ])
            .expect("run")
    } else {
        session
            .run(ort::inputs![
                "input_ids" => id_tensor,
                "attention_mask" => mask_tensor
            ])
            .expect("run")
    };
    for (name, value) in outputs.iter() {
        if let Ok((shape, _)) = value.try_extract_tensor::<f32>() {
            println!("run output {name}: {shape:?}");
        }
    }
    if !first_output.is_empty() {
        println!(
            "named get('{first_output}') present: {}",
            outputs.get(first_output.as_str()).is_some()
        );
    }
}
