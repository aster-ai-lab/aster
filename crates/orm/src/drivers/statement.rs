use crate::Value;

/// Columns and rows from one statement, independent of which bundled driver ran it.
#[derive(Clone, Debug, PartialEq)]
pub struct StatementGrid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}
