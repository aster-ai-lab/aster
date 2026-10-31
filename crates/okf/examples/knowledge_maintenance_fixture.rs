//! Test fixture access to the same public maintenance owner used by Studio.

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result = match args.as_slice() {
        [operation, root] if operation == "begin" => {
            serde_json::to_value(aster_okf::begin_knowledge_maintenance(
                Path::new(root),
                &[],
                aster_okf::MaintenanceOperation::Ingest,
            )?)?
        }
        [operation, root, job] if operation == "finish" => {
            serde_json::to_value(aster_okf::finish_knowledge_maintenance(Path::new(root), job)?)?
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "usage: knowledge_maintenance_fixture begin ROOT | finish ROOT JOB_ID",
            )
            .into())
        }
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
