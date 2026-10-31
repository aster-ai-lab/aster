macro_rules! delegate_utility_vm_runtime_driver {
    ($driver:ty, $inner:ident) => {
        #[aster_oci_sdk::async_trait]
        impl $crate::RuntimeDriver for $driver {
            fn capability(&self) -> aster_oci_core::DriverCapability {
                $crate::RuntimeDriver::capability(&self.$inner)
            }

            fn linux_support(&self) -> aster_oci_sdk::Result<aster_oci_sdk::OciLinuxSupport> {
                $crate::RuntimeDriver::linux_support(&self.$inner)
            }

            fn operations(&self) -> &[aster_oci_sdk::RuntimeOperation] {
                $crate::RuntimeDriver::operations(&self.$inner)
            }

            fn hooks(&self) -> &[$crate::OciHookPhase] {
                $crate::RuntimeDriver::hooks(&self.$inner)
            }

            fn attachment_capabilities(&self) -> aster_oci_sdk::AttachmentCapabilities {
                $crate::RuntimeDriver::attachment_capabilities(&self.$inner)
            }

            async fn acknowledge_operation(
                &self,
                operation_id: &aster_oci_sdk::OperationId,
            ) -> aster_oci_sdk::Result<()> {
                $crate::RuntimeDriver::acknowledge_operation(&self.$inner, operation_id).await
            }

            async fn prepare_create_bundle(
                &self,
                request: &$crate::DriverCreateRequest,
            ) -> aster_oci_sdk::Result<aster_oci_sdk::OciBundle> {
                $crate::RuntimeDriver::prepare_create_bundle(&self.$inner, request).await
            }

            async fn recover(
                &self,
                record: &aster_oci_sdk::ContainerRecord,
            ) -> aster_oci_sdk::Result<$crate::DriverRecovery> {
                $crate::RuntimeDriver::recover(&self.$inner, record).await
            }

            async fn create(
                &self,
                request: $crate::DriverCreateRequest,
            ) -> aster_oci_sdk::Result<$crate::DriverState> {
                $crate::RuntimeDriver::create(&self.$inner, request).await
            }

            async fn state(
                &self,
                target: aster_oci_sdk::ContainerTarget,
            ) -> aster_oci_sdk::Result<$crate::DriverState> {
                $crate::RuntimeDriver::state(&self.$inner, target).await
            }

            async fn start(
                &self,
                request: $crate::DriverStartRequest,
            ) -> aster_oci_sdk::Result<$crate::DriverState> {
                $crate::RuntimeDriver::start(&self.$inner, request).await
            }

            async fn kill(
                &self,
                request: $crate::DriverKillRequest,
            ) -> aster_oci_sdk::Result<$crate::DriverState> {
                $crate::RuntimeDriver::kill(&self.$inner, request).await
            }

            async fn delete(
                &self,
                request: $crate::DriverDeleteRequest,
            ) -> aster_oci_sdk::Result<()> {
                $crate::RuntimeDriver::delete(&self.$inner, request).await
            }

            async fn wait(
                &self,
                request: $crate::DriverWaitRequest,
            ) -> aster_oci_sdk::Result<aster_oci_sdk::ExitStatus> {
                $crate::RuntimeDriver::wait(&self.$inner, request).await
            }

            async fn exec(
                &self,
                request: $crate::DriverExecRequest,
            ) -> aster_oci_sdk::Result<$crate::DriverProcess> {
                $crate::RuntimeDriver::exec(&self.$inner, request).await
            }

            async fn signal_process(
                &self,
                request: $crate::DriverSignalProcessRequest,
            ) -> aster_oci_sdk::Result<()> {
                $crate::RuntimeDriver::signal_process(&self.$inner, request).await
            }

            async fn wait_process(
                &self,
                request: $crate::DriverWaitProcessRequest,
            ) -> aster_oci_sdk::Result<aster_oci_sdk::ExitStatus> {
                $crate::RuntimeDriver::wait_process(&self.$inner, request).await
            }

            async fn pause(
                &self,
                request: $crate::DriverContainerOperationRequest,
            ) -> aster_oci_sdk::Result<$crate::DriverState> {
                $crate::RuntimeDriver::pause(&self.$inner, request).await
            }

            async fn resume(
                &self,
                request: $crate::DriverContainerOperationRequest,
            ) -> aster_oci_sdk::Result<$crate::DriverState> {
                $crate::RuntimeDriver::resume(&self.$inner, request).await
            }

            async fn processes(
                &self,
                target: aster_oci_sdk::ContainerTarget,
            ) -> aster_oci_sdk::Result<Vec<aster_oci_sdk::ProcessRecord>> {
                $crate::RuntimeDriver::processes(&self.$inner, target).await
            }

            async fn update(
                &self,
                request: $crate::DriverUpdateRequest,
            ) -> aster_oci_sdk::Result<$crate::DriverState> {
                $crate::RuntimeDriver::update(&self.$inner, request).await
            }

            async fn stats(
                &self,
                target: aster_oci_sdk::ContainerTarget,
            ) -> aster_oci_sdk::Result<aster_oci_sdk::ContainerStats> {
                $crate::RuntimeDriver::stats(&self.$inner, target).await
            }

            async fn read_output(
                &self,
                request: $crate::DriverReadOutputRequest,
            ) -> aster_oci_sdk::Result<Vec<aster_oci_sdk::OutputChunk>> {
                $crate::RuntimeDriver::read_output(&self.$inner, request).await
            }

            async fn write_stdin(
                &self,
                request: $crate::DriverWriteStdinRequest,
            ) -> aster_oci_sdk::Result<()> {
                $crate::RuntimeDriver::write_stdin(&self.$inner, request).await
            }

            async fn close_stdin(
                &self,
                request: $crate::DriverCloseStdinRequest,
            ) -> aster_oci_sdk::Result<()> {
                $crate::RuntimeDriver::close_stdin(&self.$inner, request).await
            }

            async fn resize(
                &self,
                request: $crate::DriverResizeRequest,
            ) -> aster_oci_sdk::Result<()> {
                $crate::RuntimeDriver::resize(&self.$inner, request).await
            }

            async fn file(
                &self,
                request: aster_oci_sdk::FileRequest,
            ) -> aster_oci_sdk::Result<aster_oci_sdk::FileResponse> {
                $crate::RuntimeDriver::file(&self.$inner, request).await
            }

            async fn filesystem(
                &self,
                request: aster_oci_sdk::FilesystemRequest,
            ) -> aster_oci_sdk::Result<aster_oci_sdk::FilesystemResponse> {
                $crate::RuntimeDriver::filesystem(&self.$inner, request).await
            }
        }
    };
}

pub(crate) use delegate_utility_vm_runtime_driver;
