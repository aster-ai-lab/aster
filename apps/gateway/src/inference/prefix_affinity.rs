//! Bounded, private routing hints for successfully completed prompt prefixes.
//!
//! These are not KV residency attestations. Health, grants, protocol, priority,
//! and backend admission remain authoritative; locality only breaks ties inside
//! an eligible target group and yields when local outstanding work diverges.

use crate::config::InferenceTargetConfig;
use serde::ser::{SerializeMap, Serializer};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::io::{self, Write};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use uuid::Uuid;

const BLOCK_BYTES: usize = 4096;
const MIN_PREFIX_BYTES: usize = 1024;
const MAX_HASH_BYTES: usize = 256 * 1024;
const MAX_CHECKPOINTS: usize = 128;
const MAX_ENTRIES: usize = 4096;
const TTL: Duration = Duration::from_secs(300);
/// One extra outstanding operation per healthy backend may retain locality.
const LOAD_BAND: u64 = 1000;
const MAX_PROFILES: usize = 256;
const MAX_RESERVATIONS: usize = 4096;

type Fingerprint = [u8; 32];
type EntryKey = (Fingerprint, Fingerprint);

#[derive(Clone)]
pub(crate) struct PrefixAffinity {
    salt: Uuid,
    index: Arc<Mutex<Index>>,
}

impl Default for PrefixAffinity {
    fn default() -> Self {
        Self {
            salt: Uuid::new_v4(),
            index: Arc::new(Mutex::new(Index::default())),
        }
    }
}

#[derive(Default)]
struct Index {
    entries: HashMap<EntryKey, Entry>,
    lru: BTreeSet<(u64, EntryKey)>,
    sequence: u64,
    profiles: HashMap<Fingerprint, Performance>,
    reservations: HashMap<Uuid, Reservation>,
}

#[derive(Clone, Copy)]
struct Performance {
    observed_at: Instant,
    tokens_per_byte: f64,
    prefill_ms_per_token: f64,
    decode_ms_per_token: f64,
}

struct Reservation {
    signature: Fingerprint,
    started_at: Instant,
    estimated_ms: f64,
}

/// Upstream-reported counts only. Routing estimates never enter accounting.
pub(crate) struct PrefixTiming {
    pub(crate) prompt: u64,
    pub(crate) cached: u64,
    pub(crate) output: u64,
    pub(crate) prefill_ms: f64,
    pub(crate) decode_ms: f64,
}

/// Owned by the response, not a background task. Drop releases work on every
/// cancellation/fallback path; only a validated terminal response adds hints.
pub(crate) struct PrefixLease {
    owner: PrefixAffinity,
    hint: PrefixHint,
    signature: Fingerprint,
    id: Uuid,
    expected_match: usize,
}

impl PrefixLease {
    pub(crate) fn complete(self, timing: Option<PrefixTiming>, now: Instant) {
        self.owner.completed(&self, timing, now);
    }
}

impl Drop for PrefixLease {
    fn drop(&mut self) {
        self.owner
            .index
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reservations
            .remove(&self.id);
    }
}

struct Entry {
    accepted_at: Instant,
    sequence: u64,
}

/// Deliberately has no Debug/Serialize implementation: never log fingerprints.
#[derive(Clone)]
pub(crate) struct PrefixHint {
    checkpoints: Vec<(usize, Fingerprint)>,
    bytes: usize,
    input_bytes: usize,
    output_limit: u64,
    pub(crate) parallelism: u32,
}

pub(crate) struct PrefixTarget {
    pub(crate) target_id: Uuid,
    pub(crate) signature: Fingerprint,
    /// Outstanding operations per healthy backend, in thousandths.
    pub(crate) load: u64,
}

#[derive(Default)]
pub(crate) struct PrefixSelection {
    pub(crate) preferred: Option<Uuid>,
    pub(crate) matched_bytes: usize,
    pub(crate) least_loaded: Vec<Uuid>,
    pub(crate) estimated_ms: Option<u64>,
}

impl PrefixAffinity {
    pub(crate) fn hint(
        &self,
        scope: impl Serialize,
        document: &Map<String, Value>,
    ) -> Option<PrefixHint> {
        let messages = document.get("messages")?.as_array()?;
        if messages.is_empty() {
            return None;
        }
        let mut scope_hash = Sha256::new();
        scope_hash.update(b"a3s.gateway.prefix-affinity.v1");
        scope_hash.update(self.salt.as_bytes());
        let scope = serde_json::to_vec(&scope).ok()?;
        scope_hash.update((scope.len() as u64).to_le_bytes());
        scope_hash.update(scope);
        let mut writer = PrefixWriter::new(scope_hash);
        // These fields may affect the rendered prompt. Sampling and delivery
        // parameters do not. Unknown fields still reach the strict upstream
        // parser; their omission here never grants them execution authority.
        let context = Projection(
            document,
            &[
                "tools",
                "functions",
                "tool_choice",
                "function_call",
                "response_format",
                "parallel_tool_calls",
                "reasoning_effort",
                "prompt_cache_key",
            ],
        );
        let _ = serde_json::to_writer(&mut writer, &context);
        for message in messages {
            let object = message.as_object()?;
            // Canonical inference fields, excluding client replay annotations.
            let message = Projection(
                object,
                &[
                    "role",
                    "content",
                    "name",
                    "tool_calls",
                    "tool_call_id",
                    "images",
                    "thinking",
                    "reasoning_content",
                ],
            );
            if serde_json::to_writer(&mut writer, &message).is_err() {
                break; // The finite hashing budget was reached.
            }
            writer.checkpoint();
        }
        writer.checkpoint();
        (!writer.checkpoints.is_empty()).then_some(PrefixHint {
            parallelism: 1,
            checkpoints: writer.checkpoints,
            bytes: writer.bytes,
            input_bytes: writer.total_bytes,
            output_limit: document
                .get("max_completion_tokens")
                .or_else(|| document.get("max_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(256)
                .min(131_072),
        })
    }

    pub(crate) fn select(
        &self,
        hint: &PrefixHint,
        candidates: &[PrefixTarget],
        now: Instant,
    ) -> PrefixSelection {
        let index = self.index.lock().unwrap_or_else(PoisonError::into_inner);
        let mut work = HashMap::<Fingerprint, (u64, f64)>::new();
        for reservation in index.reservations.values() {
            let entry = work.entry(reservation.signature).or_default();
            entry.0 = entry.0.saturating_add(1000);
            entry.1 += (reservation.estimated_ms
                - now
                    .saturating_duration_since(reservation.started_at)
                    .as_secs_f64()
                    * 1000.0)
                .max(1.0);
        }
        let load = |candidate: &PrefixTarget| {
            candidate
                .load
                .max(work.get(&candidate.signature).map_or(0, |(load, _)| *load))
        };
        let minimum = candidates.iter().map(load).min().unwrap_or(0);
        let mut result = PrefixSelection {
            least_loaded: candidates
                .iter()
                .filter(|candidate| load(candidate) == minimum)
                .map(|candidate| candidate.target_id)
                .collect(),
            ..PrefixSelection::default()
        };
        let mut best = None;
        let mut costs = Vec::new();
        let fallback = candidates
            .iter()
            .filter_map(|candidate| {
                index
                    .profiles
                    .get(&candidate.signature)
                    .filter(|profile| now.saturating_duration_since(profile.observed_at) < TTL)
                    .copied()
            })
            .reduce(|a, b| Performance {
                observed_at: a.observed_at.min(b.observed_at),
                tokens_per_byte: (a.tokens_per_byte + b.tokens_per_byte) / 2.0,
                prefill_ms_per_token: (a.prefill_ms_per_token + b.prefill_ms_per_token) / 2.0,
                decode_ms_per_token: (a.decode_ms_per_token + b.decode_ms_per_token) / 2.0,
            });
        for candidate in candidates
            .iter()
            .filter(|candidate| load(candidate) <= minimum.saturating_add(LOAD_BAND))
        {
            let mut matched = 0;
            for (bytes, fingerprint) in hint.checkpoints.iter().rev() {
                // A short common header must not pin a large unrelated prompt.
                if *bytes < hint.bytes / 2 {
                    break;
                }
                let Some(entry) = index.entries.get(&(*fingerprint, candidate.signature)) else {
                    continue;
                };
                if now.saturating_duration_since(entry.accepted_at) >= TTL {
                    continue;
                }
                matched = *bytes;
                // Prefer the longest shared prefix, then the less busy target,
                // then recency. Stable identity resolves otherwise exact ties.
                let rank = (
                    *bytes,
                    std::cmp::Reverse(load(candidate)),
                    entry.sequence,
                    candidate.target_id,
                );
                if best.is_none_or(|previous| rank > previous) {
                    best = Some(rank);
                    result.preferred = Some(candidate.target_id);
                    result.matched_bytes = *bytes;
                }
                break;
            }
            if let Some(profile) = index
                .profiles
                .get(&candidate.signature)
                .filter(|profile| now.saturating_duration_since(profile.observed_at) < TTL)
                .copied()
                .or(fallback)
            {
                let estimate = estimate_ms(hint, matched, profile);
                let (known, pending) = work.get(&candidate.signature).copied().unwrap_or_default();
                // Connection load can include work from other gateway
                // processes. Never assume locally unobserved work is free.
                let unknown = candidate.load.saturating_sub(known);
                let pressure = load(candidate) as f64 / 1000.0;
                let slots = f64::from(hint.parallelism.max(1));
                // Occupying another declared slot is not a serial queue.
                // Once all slots are occupied, estimate the share of work
                // ahead of this request; this never grants admission.
                let queue_share = ((pressure + 1.0 - slots) / pressure.max(1.0)).clamp(0.0, 1.0);
                let wait = (pending + unknown as f64 / 1000.0 * estimate_ms(hint, 0, profile))
                    * queue_share
                    / slots;
                costs.push((candidate.target_id, matched, estimate + wait));
            }
        }
        if let Some(minimum_cost) = costs.iter().map(|(_, _, cost)| *cost).reduce(f64::min) {
            // A small dead band retains existing weighted tie selection;
            // timing noise must not collapse all cold traffic onto one GPU.
            let tied = costs
                .iter()
                .filter(|(_, _, cost)| *cost <= minimum_cost * 1.05 + 1.0)
                .collect::<Vec<_>>();
            result.estimated_ms = Some(minimum_cost.ceil().min(u64::MAX as f64) as u64);
            if tied.len() == 1 {
                result.preferred = Some(tied[0].0);
                result.matched_bytes = tied[0].1;
            } else if !tied.iter().any(|(id, _, _)| Some(*id) == result.preferred) {
                result.preferred = None;
                result.matched_bytes = 0;
                result.least_loaded = tied.iter().map(|(id, _, _)| *id).collect();
            }
        }
        result
    }

    pub(crate) fn reserve(
        &self,
        hint: &PrefixHint,
        signature: Fingerprint,
        matched: usize,
        now: Instant,
    ) -> Option<PrefixLease> {
        let mut index = self.index.lock().unwrap_or_else(PoisonError::into_inner);
        if index.reservations.len() >= MAX_RESERVATIONS {
            return None;
        }
        let estimated_ms = index
            .profiles
            .get(&signature)
            .filter(|profile| now.saturating_duration_since(profile.observed_at) < TTL)
            .map(|profile| estimate_ms(hint, matched, *profile))
            // A bounded bootstrap estimate, not a reported measurement.
            .unwrap_or(hint.input_bytes as f64 / 8.0 + hint.output_limit as f64 * 20.0);
        let id = Uuid::new_v4();
        index.reservations.insert(
            id,
            Reservation {
                signature,
                started_at: now,
                estimated_ms,
            },
        );
        Some(PrefixLease {
            owner: self.clone(),
            hint: hint.clone(),
            signature,
            id,
            expected_match: matched,
        })
    }

    fn completed(&self, lease: &PrefixLease, timing: Option<PrefixTiming>, now: Instant) {
        if let Some(timing) = timing.filter(|timing| timing.valid()) {
            let mut index = self.index.lock().unwrap_or_else(PoisonError::into_inner);
            if lease.expected_match > 0 && timing.cached == 0 {
                // A native miss disproves the prior locality prediction.
                // It does not prove any other prefix remains resident.
                let removed = index
                    .entries
                    .iter()
                    .filter(|((_, signature), _)| *signature == lease.signature)
                    .map(|(key, entry)| (*key, entry.sequence))
                    .collect::<Vec<_>>();
                for (key, sequence) in removed {
                    index.entries.remove(&key);
                    index.lru.remove(&(sequence, key));
                }
            }
            let fresh = Performance {
                observed_at: now,
                tokens_per_byte: timing.prompt as f64 / lease.hint.input_bytes.max(1) as f64,
                prefill_ms_per_token: timing.prefill_ms
                    / (timing.prompt - timing.cached).max(1) as f64,
                decode_ms_per_token: timing.decode_ms / timing.output.max(1) as f64,
            };
            // Tiny cached tails do not measure cold-prefill throughput;
            // one output token does not establish a decode-step rate.
            let previous = index
                .profiles
                .get(&lease.signature)
                .copied()
                .filter(|profile| now.saturating_duration_since(profile.observed_at) < TTL);
            let profile = previous
                .map(|old| Performance {
                    observed_at: now,
                    tokens_per_byte: old.tokens_per_byte * 0.75 + fresh.tokens_per_byte * 0.25,
                    prefill_ms_per_token: if timing.prompt - timing.cached >= 128 {
                        old.prefill_ms_per_token * 0.75 + fresh.prefill_ms_per_token * 0.25
                    } else {
                        old.prefill_ms_per_token
                    },
                    decode_ms_per_token: if timing.output >= 2 {
                        old.decode_ms_per_token * 0.75 + fresh.decode_ms_per_token * 0.25
                    } else {
                        old.decode_ms_per_token
                    },
                })
                .or_else(|| {
                    (timing.prompt - timing.cached >= 128 && timing.output >= 2).then_some(fresh)
                });
            if let Some(profile) = profile {
                if index.profiles.len() >= MAX_PROFILES
                    && !index.profiles.contains_key(&lease.signature)
                {
                    if let Some(key) = index
                        .profiles
                        .iter()
                        .min_by_key(|(_, profile)| profile.observed_at)
                        .map(|(key, _)| *key)
                    {
                        index.profiles.remove(&key);
                    }
                }
                index.profiles.insert(lease.signature, profile);
            }
        }
        self.accepted(&lease.hint, lease.signature, now);
    }

    pub(crate) fn accepted(&self, hint: &PrefixHint, signature: Fingerprint, now: Instant) {
        let mut index = self.index.lock().unwrap_or_else(PoisonError::into_inner);
        for (_, fingerprint) in &hint.checkpoints {
            let key = (*fingerprint, signature);
            if let Some(old) = index.entries.remove(&key) {
                index.lru.remove(&(old.sequence, key));
            }
            index.sequence = index.sequence.saturating_add(1);
            let sequence = index.sequence;
            index.entries.insert(
                key,
                Entry {
                    accepted_at: now,
                    sequence,
                },
            );
            index.lru.insert((sequence, key));
            while index.entries.len() > MAX_ENTRIES {
                if let Some((_, key)) = index.lru.pop_first() {
                    index.entries.remove(&key);
                }
            }
        }
    }
}

impl PrefixTiming {
    fn valid(&self) -> bool {
        self.prompt > 0
            && self.prompt <= 4_194_304
            && self.cached <= self.prompt
            && self.output <= 131_072
            && self.prefill_ms.is_finite()
            && self.decode_ms.is_finite()
            && self.prefill_ms > 0.0
            && self.prefill_ms <= 3_600_000.0
            && self.decode_ms >= 0.0
            && self.decode_ms <= 3_600_000.0
            && (self.output < 2 || self.decode_ms > 0.0)
    }
}

fn estimate_ms(hint: &PrefixHint, matched: usize, profile: Performance) -> f64 {
    let uncached = hint.input_bytes.saturating_sub(matched) as f64 * profile.tokens_per_byte;
    (uncached * profile.prefill_ms_per_token
        + hint.output_limit as f64 * profile.decode_ms_per_token)
        .clamp(1.0, 3_600_000.0)
}

/// Binds hints to the exact target descriptor and current backend generation.
/// The service's sorted opaque backend identities are supplied by the caller.
pub(crate) fn target_signature(
    target: &InferenceTargetConfig,
    mut backends: Vec<String>,
) -> Option<Fingerprint> {
    backends.sort_unstable();
    let bytes = serde_json::to_vec(&(target, backends)).ok()?;
    Some(Sha256::digest(bytes).into())
}

struct Projection<'a>(&'a Map<String, Value>, &'a [&'a str]);

impl Serialize for Projection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        for key in self.1 {
            if let Some(value) = self.0.get(*key) {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

struct PrefixWriter {
    hash: Sha256,
    bytes: usize,
    total_bytes: usize,
    checkpoints: Vec<(usize, Fingerprint)>,
}

impl PrefixWriter {
    fn new(hash: Sha256) -> Self {
        Self {
            hash,
            bytes: 0,
            total_bytes: 0,
            checkpoints: Vec::new(),
        }
    }

    fn checkpoint(&mut self) {
        if self.bytes >= MIN_PREFIX_BYTES
            && self.checkpoints.len() < MAX_CHECKPOINTS
            && self
                .checkpoints
                .last()
                .is_none_or(|(bytes, _)| *bytes != self.bytes)
        {
            self.checkpoints
                .push((self.bytes, self.hash.clone().finalize().into()));
        }
    }
}

impl Write for PrefixWriter {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let original_len = bytes.len();
        self.total_bytes = self.total_bytes.saturating_add(original_len);
        while !bytes.is_empty() {
            if self.bytes == MAX_HASH_BYTES {
                break;
            }
            let count = bytes
                .len()
                .min(BLOCK_BYTES - self.bytes % BLOCK_BYTES)
                .min(MAX_HASH_BYTES - self.bytes);
            self.hash.update(&bytes[..count]);
            self.bytes += count;
            bytes = &bytes[count..];
            if self.bytes % BLOCK_BYTES == 0 {
                self.checkpoint();
            }
        }
        Ok(original_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
