#![cfg(test)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Planned,
    NotRun,
    Blocked,
    Pass,
    Fail,
    Cancelled,
}

impl Status {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "planned" => Ok(Self::Planned),
            "not_run" => Ok(Self::NotRun),
            "blocked" => Ok(Self::Blocked),
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(
                "unknown status; expected planned, not_run, blocked, pass, fail, cancelled".into(),
            ),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::NotRun => "not_run",
            Self::Blocked => "blocked",
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureLayer {
    Product,
    Model,
    Provider,
    TestSpec,
    Infrastructure,
    Mixed,
    Undetermined,
}

impl FailureLayer {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "product" => Ok(Self::Product),
            "model" => Ok(Self::Model),
            "provider" => Ok(Self::Provider),
            "test_spec" => Ok(Self::TestSpec),
            "infrastructure" => Ok(Self::Infrastructure),
            "mixed" => Ok(Self::Mixed),
            "undetermined" => Ok(Self::Undetermined),
            _ => Err("unknown failure layer; expected product, model, provider, test_spec, infrastructure, mixed, undetermined".into()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Model => "model",
            Self::Provider => "provider",
            Self::TestSpec => "test_spec",
            Self::Infrastructure => "infrastructure",
            Self::Mixed => "mixed",
            Self::Undetermined => "undetermined",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    Identity,
    Oracle,
    Terminal,
    Cleanup,
    CurrentArtifact,
    FaultBarrier,
    SideEffects,
}

impl Check {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "identity" => Ok(Self::Identity),
            "oracle" => Ok(Self::Oracle),
            "terminal" => Ok(Self::Terminal),
            "cleanup" => Ok(Self::Cleanup),
            "current_artifact" => Ok(Self::CurrentArtifact),
            "fault_barrier" => Ok(Self::FaultBarrier),
            "side_effects" => Ok(Self::SideEffects),
            _ => Err("unknown check; expected identity, oracle, terminal, cleanup, current_artifact, fault_barrier, side_effects".into()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Oracle => "oracle",
            Self::Terminal => "terminal",
            Self::Cleanup => "cleanup",
            Self::CurrentArtifact => "current_artifact",
            Self::FaultBarrier => "fault_barrier",
            Self::SideEffects => "side_effects",
        }
    }
}

pub struct Definition<'a> {
    pub case: &'a str,
    pub variant: &'a str,
    pub required: &'a [Check],
}

/// One campaign binds exactly one frozen source/binary/model/platform profile.
/// This record contains host-produced evidence checks, never a model's verdict.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub campaign: String,
    pub case: String,
    pub variant: String,
    pub seed: u64,
    pub attempt: u32,
    pub started: bool,
    pub status: Status,
    pub failure_layer: Option<FailureLayer>,
    pub checks: BTreeMap<Check, bool>,
    pub tokens: Option<u64>,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub started_attempts: usize,
    pub first_attempts: usize,
    pub first_passed: usize,
    pub passed: usize,
    pub failed: usize,
    pub blocked: usize,
    pub cancelled: usize,
    pub planned: usize,
    pub not_run: usize,
    pub known_tokens: u64,
    pub unknown_usage_attempts: usize,
}

pub fn summarize(
    campaign: &str,
    definitions: &[Definition<'_>],
    records: &[Record],
) -> Result<Summary, String> {
    let mut catalog = BTreeMap::new();
    for definition in definitions {
        if catalog
            .insert((definition.case, definition.variant), definition)
            .is_some()
        {
            return Err("duplicate case/variant definition".into());
        }
    }
    let mut seen = BTreeSet::new();
    let mut summary = Summary::default();
    for record in records {
        if record.campaign != campaign {
            return Err("campaign identity mismatch".into());
        }
        let definition = catalog
            .get(&(record.case.as_str(), record.variant.as_str()))
            .ok_or("unknown case/variant")?;
        if record.attempt == 0 {
            return Err("attempt numbers start at one".into());
        }
        if !seen.insert((&record.case, &record.variant, record.seed, record.attempt)) {
            return Err("duplicate attempt identity".into());
        }
        match record.status {
            Status::Pass | Status::Fail | Status::Cancelled if !record.started => {
                return Err("a settled attempt must have started".into());
            }
            Status::Planned | Status::NotRun if record.started => {
                return Err("an unexecuted target cannot have started".into());
            }
            _ => {}
        }
        if record.status == Status::Pass {
            if record.failure_layer.is_some() {
                return Err("a passing case cannot carry a failure attribution".into());
            }
            for check in [
                Check::Identity,
                Check::Oracle,
                Check::Terminal,
                Check::Cleanup,
            ]
            .iter()
            .chain(definition.required.iter())
            {
                if record.checks.get(check) != Some(&true) {
                    return Err(format!("passing case lacks {} evidence", check.as_str()));
                }
            }
            if record.checks.values().any(|passed| !passed) {
                return Err("a passing case carries a failed check".into());
            }
        } else if matches!(record.status, Status::Fail | Status::Blocked)
            && record.failure_layer.is_none()
        {
            return Err("failed/blocked cases require an explicit attribution".into());
        }
        if record.started {
            summary.started_attempts += 1;
            if record.attempt == 1 {
                summary.first_attempts += 1;
                summary.first_passed += usize::from(record.status == Status::Pass);
            }
            match record.tokens {
                Some(tokens) => {
                    summary.known_tokens = summary
                        .known_tokens
                        .checked_add(tokens)
                        .ok_or("usage total overflow")?;
                }
                None => summary.unknown_usage_attempts += 1,
            }
        } else if record.tokens.is_some_and(|tokens| tokens != 0) {
            return Err("an unstarted target cannot consume known tokens".into());
        }
        match record.status {
            Status::Pass => summary.passed += 1,
            Status::Fail => summary.failed += 1,
            Status::Blocked => summary.blocked += 1,
            Status::Cancelled => summary.cancelled += 1,
            Status::Planned => summary.planned += 1,
            Status::NotRun => summary.not_run += 1,
        }
    }
    let mut next_attempt = BTreeMap::new();
    for (case, variant, seed, attempt) in seen {
        let expected = next_attempt.entry((case, variant, seed)).or_insert(1_u64);
        if u64::from(attempt) != *expected {
            return Err("attempt history is missing an earlier attempt".into());
        }
        *expected += 1;
    }
    Ok(summary)
}
