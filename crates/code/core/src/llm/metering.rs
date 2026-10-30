//! Invocation facts shared by local model dispatch, usage views and budgets.
//! Missing measurements stay unknown; these cost estimates are not vendor bills.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};

pub mod wire;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputAccounting {
    IncludesCache,
    ExcludesCache,
}

impl InputAccounting {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "includes_cache" => Ok(Self::IncludesCache),
            "excludes_cache" => Ok(Self::ExcludesCache),
            _ => bail!("unknown input accounting: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::IncludesCache => "includes_cache",
            Self::ExcludesCache => "excludes_cache",
        }
    }
}

/// Input includes cache reads/writes; uncached input is disjoint from both.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservedTokens {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub no_cache_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
}

impl ObservedTokens {
    /// Normalize at the provider boundary, before persisting a call's usage.
    pub fn from_reported(
        input: Option<u64>,
        output: Option<u64>,
        cache_read: Option<u64>,
        cache_write: Option<u64>,
        accounting: InputAccounting,
    ) -> Result<Self> {
        let cache = cache_read
            .zip(cache_write)
            .map(|(read, write)| checked_add(read, write))
            .transpose()?;
        let (input_tokens, no_cache_tokens) = match accounting {
            InputAccounting::IncludesCache => {
                let uncached = input
                    .zip(cache)
                    .map(|(input, cache)| {
                        input
                            .checked_sub(cache)
                            .ok_or_else(|| anyhow::anyhow!("cache tokens exceed input tokens"))
                    })
                    .transpose()?;
                if let Some(input) = input {
                    ensure!(
                        cache_read.unwrap_or(0) <= input && cache_write.unwrap_or(0) <= input,
                        "cache tokens exceed input tokens"
                    );
                }
                (input, uncached)
            }
            InputAccounting::ExcludesCache => (
                input
                    .zip(cache)
                    .map(|(input, cache)| checked_add(input, cache))
                    .transpose()?,
                input,
            ),
        };
        let tokens = Self {
            input_tokens,
            output_tokens: output,
            no_cache_tokens,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
        };
        tokens.validate()?;
        Ok(tokens)
    }

    pub fn total_tokens(&self) -> Result<Option<u64>> {
        self.input_tokens
            .zip(self.output_tokens)
            .map(|(input, output)| checked_add(input, output))
            .transpose()
    }

    pub fn validate(&self) -> Result<()> {
        self.total_tokens()?;
        let mut known_input = 0;
        for value in [
            self.no_cache_tokens,
            self.cache_read_tokens,
            self.cache_write_tokens,
        ]
        .into_iter()
        .flatten()
        {
            known_input = checked_add(known_input, value)?;
        }
        if let Some(input) = self.input_tokens {
            ensure!(known_input <= input, "input token components exceed total");
            if self.no_cache_tokens.is_some()
                && self.cache_read_tokens.is_some()
                && self.cache_write_tokens.is_some()
            {
                ensure!(known_input == input, "input token components disagree");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Currency {
    #[serde(rename = "USD")]
    Usd,
    #[serde(rename = "CNY")]
    Cny,
}

impl Currency {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "USD" => Ok(Self::Usd),
            "CNY" => Ok(Self::Cny),
            _ => bail!("unknown metering currency: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Usd => "USD",
            Self::Cny => "CNY",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenPrices {
    pub input_per_million_tokens: Option<f64>,
    pub output_per_million_tokens: Option<f64>,
    pub cache_read_per_million_tokens: Option<f64>,
    pub cache_write_per_million_tokens: Option<f64>,
}

impl TokenPrices {
    fn validate(&self) -> Result<()> {
        ensure!(
            [
                self.input_per_million_tokens,
                self.output_per_million_tokens,
                self.cache_read_per_million_tokens,
                self.cache_write_per_million_tokens,
            ]
            .into_iter()
            .flatten()
            .all(|price| price.is_finite() && price >= 0.0),
            "token prices must be finite and nonnegative"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputPriceTier {
    pub min_input_tokens: u64,
    pub prices: TokenPrices,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PricingSnapshot {
    pub currency: Currency,
    pub prices: TokenPrices,
    #[serde(default)]
    pub input_token_tiers: Vec<InputPriceTier>,
    pub captured_at_ms: u64,
}

impl PricingSnapshot {
    pub fn validate(&self) -> Result<()> {
        self.prices.validate()?;
        let mut previous = 0;
        for tier in &self.input_token_tiers {
            ensure!(
                tier.min_input_tokens > previous,
                "input pricing tier thresholds must be positive and strictly increasing"
            );
            tier.prices.validate()?;
            previous = tier.min_input_tokens;
        }
        Ok(())
    }

    pub fn estimate(&self, usage: &ObservedTokens) -> Result<Option<f64>> {
        let prices = if self.input_token_tiers.is_empty() {
            &self.prices
        } else if let Some(input) = usage.input_tokens {
            self.input_token_tiers
                .iter()
                .rev()
                .find(|tier| input >= tier.min_input_tokens)
                .map_or(&self.prices, |tier| &tier.prices)
        } else {
            return Ok(None);
        };
        let mut amount = 0.0;
        for (tokens, price) in [
            (usage.no_cache_tokens, prices.input_per_million_tokens),
            (usage.output_tokens, prices.output_per_million_tokens),
            (
                usage.cache_read_tokens,
                prices.cache_read_per_million_tokens,
            ),
            (
                usage.cache_write_tokens,
                prices.cache_write_per_million_tokens,
            ),
        ] {
            if tokens == Some(0) {
                continue;
            }
            let (Some(tokens), Some(price)) = (tokens, price) else {
                return Ok(None);
            };
            amount += (tokens as f64 / 1_000_000.0) * price;
        }
        ensure!(amount.is_finite(), "estimated cost overflow");
        Ok(Some(amount))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl AttemptStatus {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => bail!("unknown model attempt status: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationIdentity {
    pub request_id: String,
    pub attempt_id: String,
    pub source_id: String,
    pub session_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub credential_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationBounds {
    pub tokens: Option<u64>,
    pub cost: Option<CostBound>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CostBound {
    pub currency: Currency,
    pub micro_units: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationStart {
    pub identity: InvocationIdentity,
    pub gateway_client_id: Option<String>,
    pub started_at_ms: u64,
    pub pricing: Option<PricingSnapshot>,
    pub reference_price: bool,
    #[serde(default)]
    pub routing: Option<InvocationRoute>,
    /// Bounds are supplied by the admitted provider adapter, never a gateway caller.
    pub bounds: InvocationBounds,
}

impl InvocationStart {
    pub fn validate(&self) -> Result<()> {
        let identity = &self.identity;
        validate_ids(
            [
                &identity.request_id,
                &identity.attempt_id,
                &identity.source_id,
                &identity.provider_id,
                &identity.model_id,
            ]
            .into_iter()
            .chain(identity.session_id.iter())
            .chain(identity.credential_id.iter())
            .chain(self.gateway_client_id.iter())
            .chain(self.routing.iter().flat_map(|route| {
                std::iter::once(&route.route_id).chain(route.prior_attempt_id.iter())
            })),
        )?;
        if let Some(pricing) = &self.pricing {
            pricing.validate()?;
        }
        if let Some(bound) = &self.bounds.cost {
            ensure!(
                !self.reference_price,
                "subscription reference price is not a cash bound"
            );
            let pricing = self
                .pricing
                .as_ref()
                .context("cash bound requires a pricing snapshot")?;
            ensure!(
                bound.currency == pricing.currency,
                "cash bound currency disagrees with pricing"
            );
        }
        Ok(())
    }

    pub fn complete(&self, result: InvocationResult) -> Result<InvocationRecord> {
        let identity = &self.identity;
        let record = InvocationRecord {
            request_id: identity.request_id.clone(),
            attempt_id: identity.attempt_id.clone(),
            source_id: identity.source_id.clone(),
            session_id: identity.session_id.clone(),
            provider_id: identity.provider_id.clone(),
            model_id: identity.model_id.clone(),
            credential_id: identity.credential_id.clone(),
            response_model: result.response_model,
            status: result.status,
            started_at_ms: self.started_at_ms,
            ended_at_ms: result.ended_at_ms,
            first_token_ms: result.first_token_ms,
            usage: result.usage,
            usage_complete: result.usage_complete,
            http_status: result.http_status,
            failure: result.failure,
            pricing: self.pricing.clone(),
            reference_price: self.reference_price,
        };
        record.validate()?;
        Ok(record)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationRoute {
    pub route_id: String,
    pub candidate_index: u32,
    pub prior_attempt_id: Option<String>,
    #[serde(default)]
    pub skipped_candidates: Vec<RouteCandidateRefusal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteCandidateRefusal {
    pub model: super::local_policy::ModelReference,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptFailure {
    Transport,
    Upstream,
    Stream,
    InvalidResponse,
    Cancelled,
    Interrupted,
}

impl AttemptFailure {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "transport" => Ok(Self::Transport),
            "upstream" => Ok(Self::Upstream),
            "stream" => Ok(Self::Stream),
            "invalid_response" => Ok(Self::InvalidResponse),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => bail!("unknown model attempt failure: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transport => "transport",
            Self::Upstream => "upstream",
            Self::Stream => "stream",
            Self::InvalidResponse => "invalid_response",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationResult {
    pub status: AttemptStatus,
    pub ended_at_ms: u64,
    pub first_token_ms: Option<u64>,
    pub usage: ObservedTokens,
    pub usage_complete: bool,
    #[serde(default)]
    pub http_status: Option<u16>,
    #[serde(default)]
    pub failure: Option<AttemptFailure>,
    pub response_model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationRecord {
    pub request_id: String,
    pub attempt_id: String,
    pub source_id: String,
    pub session_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub credential_id: Option<String>,
    pub response_model: Option<String>,
    pub status: AttemptStatus,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub first_token_ms: Option<u64>,
    pub usage: ObservedTokens,
    /// Streaming counters can be cumulative lower bounds until final usage arrives.
    pub usage_complete: bool,
    #[serde(default)]
    pub http_status: Option<u16>,
    #[serde(default)]
    pub failure: Option<AttemptFailure>,
    pub pricing: Option<PricingSnapshot>,
    /// Subscription model prices are reference values, not cash spend.
    pub reference_price: bool,
}

impl InvocationRecord {
    pub fn validate(&self) -> Result<()> {
        validate_ids(
            [
                &self.request_id,
                &self.attempt_id,
                &self.source_id,
                &self.provider_id,
                &self.model_id,
            ]
            .into_iter()
            .chain(self.session_id.iter())
            .chain(self.credential_id.iter())
            .chain(self.response_model.iter()),
        )?;
        let elapsed = self
            .ended_at_ms
            .checked_sub(self.started_at_ms)
            .ok_or_else(|| anyhow::anyhow!("invocation ended before it started"))?;
        if let Some(first) = self.first_token_ms {
            ensure!(first <= elapsed, "first token exceeds invocation duration");
        }
        self.usage.validate()?;
        ensure!(
            self.http_status
                .is_none_or(|status| (100..=599).contains(&status)),
            "invalid model attempt HTTP status"
        );
        if let Some(pricing) = &self.pricing {
            pricing.validate()?;
            pricing.estimate(&self.usage)?;
        }
        Ok(())
    }

    pub fn estimated_cost(&self) -> Result<Option<f64>> {
        if !self.usage_complete {
            return Ok(None);
        }
        match &self.pricing {
            Some(pricing) => pricing.estimate(&self.usage),
            None => Ok(None),
        }
    }

    pub fn output_tokens_per_second(&self) -> Option<f64> {
        if !self.usage_complete {
            return None;
        }
        let first = self.first_token_ms?;
        let generation = self
            .ended_at_ms
            .checked_sub(self.started_at_ms)?
            .checked_sub(first)?;
        if generation == 0 {
            return None;
        }
        Some(self.usage.output_tokens? as f64 * 1000.0 / generation as f64)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyTotals {
    pub estimated_cost: f64,
    pub reference_value: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvocationTotals {
    pub requests: u64,
    pub attempts: u64,
    pub known_total_tokens: u64,
    pub unknown_usage_attempts: u64,
    pub unpriced_attempts: u64,
    pub costs: BTreeMap<Currency, CurrencyTotals>,
}

impl InvocationTotals {
    /// Records have already been validated by their ingestion boundary.
    pub fn from_records<'a>(
        records: impl IntoIterator<Item = &'a InvocationRecord>,
    ) -> Result<Self> {
        let mut totals = Self::default();
        let mut requests = BTreeSet::new();
        let mut attempts = BTreeMap::new();
        for record in records {
            if let Some(previous) = attempts.insert(&record.attempt_id, record) {
                ensure!(previous == record, "conflicting invocation attempt replay");
                continue;
            }
            requests.insert(&record.request_id);
            totals.attempts = checked_add(totals.attempts, 1)?;
            match record
                .usage
                .total_tokens()?
                .filter(|_| record.usage_complete)
            {
                Some(tokens) => {
                    totals.known_total_tokens = checked_add(totals.known_total_tokens, tokens)?;
                }
                None => totals.unknown_usage_attempts += 1,
            }
            match record.estimated_cost()? {
                Some(amount) => {
                    if let Some(pricing) = &record.pricing {
                        let cost = totals.costs.entry(pricing.currency).or_default();
                        let value = if record.reference_price {
                            &mut cost.reference_value
                        } else {
                            &mut cost.estimated_cost
                        };
                        *value += amount;
                        ensure!(value.is_finite(), "aggregate estimated cost overflow");
                    }
                }
                None => totals.unpriced_attempts += 1,
            }
        }
        totals.requests = requests.len() as u64;
        Ok(totals)
    }
}

fn validate_ids<'a>(ids: impl IntoIterator<Item = &'a String>) -> Result<()> {
    for id in ids {
        ensure!(
            !id.trim().is_empty() && id.len() <= 1024,
            "invocation identity must contain 1 to 1024 bytes"
        );
    }
    Ok(())
}

fn checked_add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| anyhow::anyhow!("metered token count overflow"))
}

#[cfg(test)]
mod tests;
