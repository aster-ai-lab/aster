//! Explicit replica-local layer placement and request-local peer snapshots.
//! This is a correctness pipeline, not TP/EP collectives or a speedup claim.

use crate::{
    layers::{
        xq::{check_cancel, invalid},
        CudaLayerDevice,
    },
    model::topology::TextArchitecture,
};
use aster_power::{
    error::Result,
    inference::{CudaPeerCopyReceipt, CudaPeerGroup, CudaPeerSession, InferenceLimits},
};
use candle_core::{Tensor, TensorId};
use std::collections::{BTreeSet, HashMap};
use tokio_util::sync::CancellationToken;

/// Supported complete-model execution group widths on an eight-GPU host.
/// Physical UUID allocation is bound independently by the process/runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XqReplicaWidth {
    Two,
    Four,
    Eight,
}
impl XqReplicaWidth {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "2" => Ok(Self::Two),
            "4" => Ok(Self::Four),
            "8" => Ok(Self::Eight),
            _ => Err(invalid("XQ replica width requires 2, 4 or 8 CUDA devices")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Two => "2",
            Self::Four => "4",
            Self::Eight => "8",
        }
    }
    pub fn device_count(self) -> usize {
        match self {
            Self::Two => 2,
            Self::Four => 4,
            Self::Eight => 8,
        }
    }
}

/// Explicit mapping from model-local ranks to CUDA ordinals in this process.
/// A composition root must separately ensure that live replicas do not overlap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XqDeviceMap {
    ordinals: Vec<usize>,
}
impl XqDeviceMap {
    pub fn new(width: XqReplicaWidth, ordinals: Vec<usize>) -> Result<Self> {
        if ordinals.len() != width.device_count()
            || ordinals.iter().any(|&ordinal| ordinal >= 8)
            || ordinals.iter().copied().collect::<BTreeSet<_>>().len() != ordinals.len()
        {
            return Err(invalid("XQ device map requires the declared 2/4/8 distinct process CUDA ordinals within 0..8"));
        }
        Ok(Self { ordinals })
    }
    pub fn identity(width: XqReplicaWidth) -> Self {
        Self {
            ordinals: (0..width.device_count()).collect(),
        }
    }
    pub fn ordinals(&self) -> &[usize] {
        &self.ordinals
    }
    pub fn local_rank(&self, ordinal: usize) -> Result<usize> {
        self.ordinals
            .iter()
            .position(|&candidate| candidate == ordinal)
            .ok_or_else(|| invalid("CUDA ordinal is outside the explicit XQ replica device map"))
    }
    pub fn peer_edges(&self, local: &[(usize, usize)]) -> Result<Vec<(usize, usize)>> {
        local
            .iter()
            .map(|&(source, target)| {
                let source = self.ordinals.get(source).copied();
                let target = self.ordinals.get(target).copied();
                match (source, target) {
                    (Some(source), Some(target)) if source != target => Ok((source, target)),
                    _ => Err(invalid("XQ peer edge lies outside its explicit device map")),
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XqPlacement {
    width: XqReplicaWidth,
    layers: Vec<usize>,
    embedding: usize,
    head: usize,
}

impl XqPlacement {
    pub fn new(layers: Vec<usize>, embedding: usize, head: usize) -> Result<Self> {
        Self::for_replica(XqReplicaWidth::Eight, layers, embedding, head)
    }
    pub fn for_replica(
        width: XqReplicaWidth,
        layers: Vec<usize>,
        embedding: usize,
        head: usize,
    ) -> Result<Self> {
        let devices = width.device_count();
        if layers.len() != 40
            || layers.iter().any(|&rank| rank >= devices)
            || embedding >= devices
            || head >= devices
            || layers.iter().copied().collect::<BTreeSet<_>>() != (0..devices).collect()
        {
            return Err(invalid("XQ pipeline placement requires 40 layers using every declared replica-local ordinal, and bound embedding/head ordinals"));
        }
        Ok(Self {
            width,
            layers,
            embedding,
            head,
        })
    }
    pub fn five_layers_per_device() -> Self {
        Self::balanced(XqReplicaWidth::Eight)
    }
    pub fn balanced(width: XqReplicaWidth) -> Self {
        let devices = width.device_count();
        Self {
            width,
            layers: (0..40).map(|layer| layer * devices / 40).collect(),
            embedding: 0,
            head: devices - 1,
        }
    }
    pub fn device_count(&self) -> usize {
        self.width.device_count()
    }
    pub fn replica_width(&self) -> XqReplicaWidth {
        self.width
    }
    pub fn layer_ordinals(&self) -> &[usize] {
        &self.layers
    }
    pub fn embedding_ordinal(&self) -> usize {
        self.embedding
    }
    pub fn head_ordinal(&self) -> usize {
        self.head
    }
    pub fn peer_edges(&self, architecture: &TextArchitecture) -> Result<Vec<(usize, usize)>> {
        let mut edges = BTreeSet::new();
        let mut previous = self.embedding;
        for binding in architecture.layers() {
            let current = *self
                .layers
                .get(binding.layer())
                .ok_or_else(|| invalid("XQ placement lost a backbone layer"))?;
            if current != previous {
                edges.insert((previous, current));
            }
            previous = current;
            if let Some(source) = binding.sources() {
                for source in [source.kv(), source.index_k()] {
                    let owner = self.layers[source];
                    if owner != current {
                        edges.insert((owner, current));
                    }
                }
            }
        }
        if previous != self.head {
            edges.insert((previous, self.head));
        }
        Ok(edges.into_iter().collect())
    }
}

/// Only private immutable prepared-cache owners use the memoized path. The
/// cache ends with this request step, including rejected speculative rows.
pub(crate) struct XqStepTransfers<'a> {
    group: Option<&'a CudaPeerGroup>,
    session: Option<CudaPeerSession<'a>>,
    frozen: HashMap<(TensorId, usize), Tensor>,
    limits: InferenceLimits,
    failed: bool,
    max_frozen_copies: usize,
}

impl<'a> XqStepTransfers<'a> {
    pub(crate) fn new(
        group: &'a CudaPeerGroup,
        limits: &InferenceLimits,
        max_bytes: u64,
        max_copies: usize,
    ) -> Result<Self> {
        if group.limits() != limits {
            return Err(invalid("XQ peer group and step resource limits differ"));
        }
        Ok(Self {
            group: Some(group),
            session: Some(group.begin(max_bytes, max_copies)?),
            frozen: HashMap::with_capacity(max_copies),
            limits: limits.clone(),
            failed: false,
            max_frozen_copies: max_copies,
        })
    }
    pub(crate) fn local(limits: &InferenceLimits) -> Self {
        Self {
            group: None,
            session: None,
            frozen: HashMap::new(),
            limits: limits.clone(),
            failed: false,
            max_frozen_copies: 0,
        }
    }
    pub(crate) fn poison(&mut self) {
        self.failed = true;
    }
    pub(crate) fn validate_complete(&self, cancellation: &CancellationToken) -> Result<()> {
        check_cancel(cancellation)?;
        if self.failed {
            return Err(invalid(
                "XQ peer step was poisoned by an earlier execution failure",
            ));
        }
        if let Some(session) = &self.session {
            session.validate_complete(cancellation)?;
        }
        Ok(())
    }
    pub(crate) fn receipts(&self) -> &[CudaPeerCopyReceipt] {
        self.session.as_ref().map_or(&[], CudaPeerSession::receipts)
    }
    pub(crate) fn copy(
        &mut self,
        tensor: &Tensor,
        target: &CudaLayerDevice,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        let result = self.copy_inner(tensor, target, cancellation);
        if result.is_err() {
            self.poison();
        }
        result
    }
    fn copy_inner(
        &mut self,
        tensor: &Tensor,
        target: &CudaLayerDevice,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        self.validate_complete(cancellation)?;
        if target.limits() != &self.limits {
            return Err(invalid(
                "XQ peer target resource limits differ from the request step",
            ));
        }
        if tensor.device().same_device(target.tensor_device()) {
            return Ok(tensor.clone().detach());
        }
        let group = self
            .group
            .ok_or_else(|| invalid("XQ cross-device tensor requires an explicit CUDA peer step"))?;
        let ordinal = group.ordinal_for_device(target.tensor_device())?;
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| invalid("XQ peer session is missing"))?;
        session.copy(tensor, ordinal, cancellation)
    }
    pub(crate) fn copy_frozen(
        &mut self,
        tensor: &Tensor,
        target: &CudaLayerDevice,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        let result = self.copy_frozen_inner(tensor, target, cancellation);
        if result.is_err() {
            self.poison();
        }
        result
    }
    fn copy_frozen_inner(
        &mut self,
        tensor: &Tensor,
        target: &CudaLayerDevice,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        self.validate_complete(cancellation)?;
        if target.limits() != &self.limits {
            return Err(invalid(
                "XQ shared-cache target resource limits differ from the request step",
            ));
        }
        if tensor.device().same_device(target.tensor_device()) {
            return self.copy(tensor, target, cancellation);
        }
        let group = self
            .group
            .ok_or_else(|| invalid("XQ shared cache requires a declared peer group"))?;
        let ordinal = group.ordinal_for_device(target.tensor_device())?;
        let key = (tensor.id(), ordinal);
        if let Some(copy) = self.frozen.get(&key) {
            return Ok(copy.clone());
        }
        let metadata = (self.frozen.len() + 1)
            .checked_mul(256)
            .ok_or_else(|| invalid("XQ peer cache metadata overflowed"))?;
        if self.frozen.len() >= self.max_frozen_copies || metadata > self.limits.max_input_bytes {
            return Err(invalid("XQ peer cache exceeds host metadata/count budget"));
        }
        let copy = self.copy(tensor, target, cancellation)?;
        self.frozen.insert(key, copy.clone());
        Ok(copy)
    }
}

#[cfg(test)]
mod tests;
