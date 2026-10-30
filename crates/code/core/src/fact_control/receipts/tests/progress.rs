#![cfg(test)]

use super::*;

#[test]
fn replaying_original_receipts_cannot_inflate_progress_measurement() {
    let prepared = preparation("original");
    let receipt = result(&prepared, ToolExecutionOutcome::Succeeded);
    let mut ledger = crate::harness_loop::MutationLedger::default();
    for _ in 0..4 {
        ledger.begin_receipt_replay();
        ledger.observe_tool_receipt("bash", &receipt);
    }
    assert_eq!(ledger.take_slice_delta().information_delta, 1);
}
