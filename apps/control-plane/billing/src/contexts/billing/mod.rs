//! Billing bounded context.
//!
//! domain owns prices, aggregation, and invoice totals.
//! application runs ingestion and period billing.
//! infrastructure stores those facts with aster-orm.
//! interfaces exposes them through aster-boot.

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

    use super::application::service::BillingService;
    use super::interfaces::http::{BillingController, InternalGuard};

    pub struct BillingModule {
        pub billing: Arc<BillingService>,
        pub internal: InternalGuard,
    }

    impl Module for BillingModule {
        fn name(&self) -> &'static str {
            "billing"
        }

        fn providers(&self) -> Result<Vec<ProviderDefinition>> {
            let billing = Arc::clone(&self.billing);
            let internal = self.internal.clone();
            Ok(vec![ProviderDefinition::factory::<BillingController, _>(
                move |_| {
                    Ok(BillingController {
                        billing: Arc::clone(&billing),
                        internal: internal.clone(),
                    })
                },
            )])
        }

        fn controllers(
            &self,
            module_ref: &ModuleRef,
        ) -> Result<Vec<aster_boot::ControllerDefinition>> {
            Ok(vec![module_ref.get::<BillingController>()?.controller()?])
        }
    }
}

#[cfg(feature = "server")]
pub use server_module::BillingModule;
