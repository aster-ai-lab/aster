//! Caller-authorized host service bindings, independent of IP networking.

use crate::BoxRecord;
use aster_box_core::{validate_host_service_sockets, HostServiceSocket};
use aster_runtime::contract::{RuntimeHostService, RuntimeUnitSpec};
use aster_runtime::{RuntimeError, RuntimeResult};
use async_trait::async_trait;
use std::sync::Arc;

/// The authenticated host owns reference authorization and private paths.
/// Rebinding a reference requires a new reference and specification digest.
#[async_trait]
pub trait BoxHostServiceResolver: Send + Sync {
    async fn resolve(
        &self,
        spec: &RuntimeUnitSpec,
        service: &RuntimeHostService,
    ) -> RuntimeResult<HostServiceSocket>;
}

#[cfg(test)]
#[path = "host_services_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct HostServiceOwner {
    resolver: Option<Arc<dyn BoxHostServiceResolver>>,
}

impl HostServiceOwner {
    pub(super) fn new(resolver: Arc<dyn BoxHostServiceResolver>) -> Self {
        Self {
            resolver: Some(resolver),
        }
    }
    pub(super) fn configured(&self) -> bool {
        self.resolver.is_some()
    }

    pub(super) async fn resolve(
        &self,
        spec: &RuntimeUnitSpec,
    ) -> RuntimeResult<Vec<HostServiceSocket>> {
        spec.validate().map_err(RuntimeError::InvalidRequest)?;
        if spec.network.host_services.is_empty() {
            return Ok(Vec::new());
        }
        let resolver = self.resolver.as_ref().ok_or_else(|| {
            RuntimeError::UnsupportedCapabilities(vec!["feature:HostServices".into()])
        })?;
        let mut bindings = Vec::with_capacity(spec.network.host_services.len());
        for service in &spec.network.host_services {
            let binding = resolver.resolve(spec, service).await?;
            if binding.guest_port != service.vsock_port {
                return Err(RuntimeError::Protocol(
                    "Host service resolver changed the requested VSOCK port".into(),
                ));
            }
            binding
                .validate_host_path(unsafe { libc::geteuid() })
                .map_err(|error| RuntimeError::ProviderUnavailable(error.to_string()))?;
            bindings.push(binding);
        }
        validate_host_service_sockets(&bindings, None)
            .map_err(|error| RuntimeError::InvalidRequest(error.to_string()))?;
        Ok(bindings)
    }

    pub(super) async fn authorize_start(
        &self,
        spec: &RuntimeUnitSpec,
        record: &BoxRecord,
    ) -> RuntimeResult<()> {
        let bindings = self.resolve(spec).await?;
        let actual = &record
            .managed_execution
            .as_ref()
            .ok_or_else(|| {
                RuntimeError::Protocol("Host service execution lost managed metadata".into())
            })?
            .request
            .config
            .host_services;
        if actual != &bindings {
            return Err(RuntimeError::Protocol(
                "Host service binding changed within the Runtime generation".into(),
            ));
        }
        Ok(())
    }
}
