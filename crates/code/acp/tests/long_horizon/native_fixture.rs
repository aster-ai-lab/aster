#![cfg(test)]

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const README: &str = "Maintain report.json from the CURRENT order.json and policy.json. Use native file read/edit tools only; do not run commands or code. Report exactly these JSON fields: revision, currency, units, subtotal_cents, discount_cents, tax_cents, total_cents. revision copies the current integer order.revision; currency copies order.currency; units is the integer SUM OF ALL ITEM QUANTITIES (not a currency denomination). All monetary fields are nonnegative integer cents. Sum quantity * unit_cents for subtotal_cents. Round discounts and tax to the nearest cent with halves rounded up. Apply the discount to the subtotal BEFORE computing tax. Use basis points / 10000. Preserve README.md and notes.txt exactly. Every user turn may change source data; read the current source again.\n";
const NOTES: &str = "Unrelated user draft: preserve this exact text.\n";

pub struct NativeFixture {
    pub workspace: PathBuf,
}

impl NativeFixture {
    pub fn create(root: &Path) -> std::io::Result<Self> {
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace)?;
        std::fs::write(workspace.join("README.md"), README)?;
        std::fs::write(workspace.join("notes.txt"), NOTES)?;
        Ok(Self { workspace })
    }

    pub fn prepare(&self, revision: u64) -> std::io::Result<Value> {
        let quantity = if revision == 1 { 2 } else { 5 };
        let price = if revision < 3 { 99 } else { 109 };
        let rate = if revision == 3 { 0 } else { 750 };
        let discount = if revision < 4 { 0 } else { 500 };
        let currency = if revision < 5 { "USD" } else { "EUR" };
        std::fs::write(self.workspace.join("order.json"), json!({"revision":revision,"currency":currency,"items":[{"sku":"cedar","quantity":quantity,"unit_cents":125},{"sku":"maple","quantity":3,"unit_cents":price}]}).to_string())?;
        std::fs::write(
            self.workspace.join("policy.json"),
            json!({"tax_bps":rate,"discount_bps":discount}).to_string(),
        )?;
        let subtotal = quantity * 125 + 3 * price;
        let discount_cents = (subtotal * discount + 5000) / 10000;
        let tax = ((subtotal - discount_cents) * rate + 5000) / 10000;
        Ok(
            json!({"revision":revision,"currency":currency,"units":quantity+3,"subtotal_cents":subtotal,"discount_cents":discount_cents,"tax_cents":tax,"total_cents":subtotal-discount_cents+tax}),
        )
    }

    pub fn read(&self, name: &str) -> std::io::Result<Vec<u8>> {
        super::artifact::read(&self.workspace, name)
    }

    pub fn check(&self, expected: &Value) -> bool {
        self.read("report.json")
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .as_ref()
            == Some(expected)
            && self.read("README.md").ok().as_deref() == Some(README.as_bytes())
            && self.read("notes.txt").ok().as_deref() == Some(NOTES.as_bytes())
    }
}

#[path = "native_fixture_tests.rs"]
mod tests;
