//! F64 C1/C2 state oracle. This retains a small complete history to make
//! partial-acceptance rollback observable; it is not the native ring layout.

use super::{checked_shape, finite, invalid, softmax, ReferenceMatrix, MAX_REFERENCE_ELEMENTS};
use aster_power::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCompressionKind {
    C1,
    C2,
}

impl ReferenceCompressionKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "c1" => Ok(Self::C1),
            "c2" => Ok(Self::C2),
            _ => Err(invalid("reference compression kind must be c1 or c2")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::C1 => "c1",
            Self::C2 => "c2",
        }
    }

    fn ratio(self) -> usize {
        match self {
            Self::C1 => 1,
            Self::C2 => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceCompressionState {
    kind: ReferenceCompressionKind,
    width: usize,
    capacity_tokens: usize,
    position: usize,
    scores: Vec<f64>,
    latents: Vec<f64>,
    compressed: Vec<f64>,
}

/// Each input row has one result. C2 emits only at the end of a pair.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceCompressionStep {
    pub state: ReferenceCompressionState,
    pub emissions: Vec<Option<Vec<f64>>>,
}

impl ReferenceCompressionState {
    pub fn new(
        kind: ReferenceCompressionKind,
        width: usize,
        capacity_tokens: usize,
    ) -> Result<Self> {
        let elements = checked_shape(capacity_tokens, width)?;
        // Include the complete C2 score/latent history and compressed rows.
        // Temporary snapshots and the returned emissions are separate bounded
        // oracle allocations, not a production HBM admission calculation.
        let multiplier = if kind == ReferenceCompressionKind::C2 {
            3
        } else {
            1
        };
        if elements
            .checked_mul(multiplier)
            .is_none_or(|total| total > MAX_REFERENCE_ELEMENTS)
        {
            return Err(invalid(
                "reference compression history exceeds the scalar bound",
            ));
        }
        Ok(Self {
            kind,
            width,
            capacity_tokens,
            position: 0,
            scores: Vec::new(),
            latents: Vec::new(),
            compressed: Vec::new(),
        })
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn compressed_rows(&self) -> usize {
        self.compressed.len() / self.width
    }

    pub fn compressed_values(&self) -> &[f64] {
        &self.compressed
    }

    /// Produce a tentative next state. Validation/arithmetic failure cannot
    /// change the input state; the caller commits by taking the returned state.
    pub fn extend(
        &self,
        start_position: usize,
        scores: Option<&ReferenceMatrix>,
        latents: &ReferenceMatrix,
    ) -> Result<ReferenceCompressionStep> {
        if start_position != self.position || latents.columns != self.width {
            return Err(invalid(
                "compression position or latent width does not match its state",
            ));
        }
        let end = start_position
            .checked_add(latents.rows)
            .ok_or_else(|| invalid("compression token position overflowed"))?;
        if end > self.capacity_tokens {
            return Err(invalid("compression chunk exceeds the token capacity"));
        }
        match (self.kind, scores) {
            (ReferenceCompressionKind::C1, None) => {}
            (ReferenceCompressionKind::C2, Some(scores))
                if scores.rows == latents.rows && scores.columns == self.width => {}
            _ => {
                return Err(invalid(
                    "C1 requires no scores; C2 requires matching score rows",
                ))
            }
        }
        let mut next = self.clone();
        let mut emissions = Vec::with_capacity(latents.rows);
        for row in 0..latents.rows {
            let start = row * self.width;
            let latent = &latents.values[start..start + self.width];
            let emission = match self.kind {
                ReferenceCompressionKind::C1 => Some(latent.to_vec()),
                ReferenceCompressionKind::C2 => {
                    // The score shape was validated above, before mutation.
                    let scores = scores.ok_or_else(|| invalid("C2 scores are missing"))?;
                    next.scores
                        .extend_from_slice(&scores.values[start..start + self.width]);
                    next.latents.extend_from_slice(latent);
                    if next.position % 2 == 1 {
                        Some(next.complete_pair()?)
                    } else {
                        None
                    }
                }
            };
            if let Some(values) = &emission {
                next.compressed.extend_from_slice(values);
            }
            next.position += 1;
            emissions.push(emission);
        }
        Ok(ReferenceCompressionStep {
            state: next,
            emissions,
        })
    }

    /// Retain exactly an accepted prefix, including an unfinished C2 pair.
    /// Future rows and already emitted rejected blocks cannot remain visible.
    pub fn prefix(&self, accepted_tokens: usize) -> Result<Self> {
        if accepted_tokens > self.position {
            return Err(invalid(
                "accepted compression prefix exceeds the computed position",
            ));
        }
        let mut next = self.clone();
        next.position = accepted_tokens;
        next.scores.truncate(accepted_tokens * self.width);
        next.latents.truncate(accepted_tokens * self.width);
        next.compressed
            .truncate(accepted_tokens / self.kind.ratio() * self.width);
        Ok(next)
    }

    fn complete_pair(&self) -> Result<Vec<f64>> {
        // Called only after appending the second row of a validated C2 pair.
        let end = self.latents.len();
        let left = end - 2 * self.width;
        let right = end - self.width;
        let mut output = Vec::with_capacity(self.width);
        for channel in 0..self.width {
            let weights = softmax(&[self.scores[left + channel], self.scores[right + channel]])?;
            let first = finite(weights[0] * self.latents[left + channel])?;
            let second = finite(weights[1] * self.latents[right + channel])?;
            output.push(finite(first + second)?);
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests;
