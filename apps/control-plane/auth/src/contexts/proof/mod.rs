//! Human proof bounded context.
//!
//! domain decides admission, lockout, and attestation rules.
//! application runs login, sessions, and OIDC.
//! infrastructure stores proof and calls the directory.
//! interfaces exposes that application through aster-boot.

pub mod directory_contract;
pub mod domain;

#[cfg(feature = "server")]
pub mod application;
#[cfg(feature = "server")]
pub mod infrastructure;
#[cfg(feature = "server")]
pub mod interfaces;

#[cfg(feature = "server")]
mod server_module {
    use std::sync::Arc;

    use aster_boot::{Module, ModuleRef, ProviderDefinition, Result};

    use super::application::service::AuthService;
    use super::interfaces::http::{InternalGuard, ProofController};

    pub struct ProofModule {
        pub auth: Arc<AuthService>,
        pub internal: InternalGuard,
    }

    impl Module for ProofModule {
        fn name(&self) -> &'static str {
            "proof"
        }

        fn providers(&self) -> Result<Vec<ProviderDefinition>> {
            let auth = Arc::clone(&self.auth);
            let internal = self.internal.clone();
            Ok(vec![ProviderDefinition::factory::<ProofController, _>(
                move |_| {
                    Ok(ProofController {
                        auth: Arc::clone(&auth),
                        internal: internal.clone(),
                    })
                },
            )])
        }

        fn controllers(
            &self,
            module_ref: &ModuleRef,
        ) -> Result<Vec<aster_boot::ControllerDefinition>> {
            Ok(vec![module_ref.get::<ProofController>()?.controller()?])
        }
    }
}

#[cfg(feature = "server")]
pub use server_module::ProofModule;
