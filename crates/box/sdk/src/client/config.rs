/// Runtime-backed SDK client for local aster-box state.
#[derive(Clone)]
pub struct AsterBoxClient {
    paths: AsterBoxPaths,
    image_cache_size: u64,
    execution_manager: Arc<dyn ExecutionManager>,
    execution_session_manager: Option<Arc<dyn ExecutionSessionManager>>,
}

impl std::fmt::Debug for AsterBoxClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AsterBoxClient")
            .field("paths", &self.paths)
            .field("image_cache_size", &self.image_cache_size)
            .finish_non_exhaustive()
    }
}

impl AsterBoxClient {
    /// Create a client for the default aster-box home.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a client rooted at a custom aster-box home directory.
    pub fn from_home(home: impl Into<PathBuf>) -> Self {
        Self::with_paths(AsterBoxPaths::from_home(home))
    }

    /// Create a client using explicit state paths.
    pub fn with_paths(paths: AsterBoxPaths) -> Self {
        let local_execution_manager = Arc::new(
            aster_box_runtime::LocalExecutionManager::with_vm_backend(
                paths.boxes_file.clone(),
                paths.home.clone(),
            ),
        );
        Self {
            paths,
            image_cache_size: aster_box_runtime::DEFAULT_IMAGE_CACHE_SIZE,
            execution_manager: local_execution_manager.clone(),
            execution_session_manager: Some(local_execution_manager),
        }
    }

    /// Create a client that honors the explicit process-wide OCI migration
    /// setting. With no opt-in this is equivalent to [`Self::with_paths`].
    pub async fn with_configured_paths(paths: AsterBoxPaths) -> Result<Self> {
        let local_execution_manager = Arc::new(
            aster_box_runtime::LocalExecutionManager::with_configured_backend(
                paths.boxes_file.clone(),
                paths.home.clone(),
            )
            .await?,
        );
        Ok(Self {
            paths,
            image_cache_size: aster_box_runtime::DEFAULT_IMAGE_CACHE_SIZE,
            execution_manager: local_execution_manager.clone(),
            execution_session_manager: Some(local_execution_manager),
        })
    }

    /// Create a client with a programmatic native-Linux Sandbox migration
    /// configuration, independent of process environment parsing.
    pub async fn with_native_linux_oci_migration(
        paths: AsterBoxPaths,
        config: aster_box_runtime::NativeLinuxOciMigrationConfig,
    ) -> Result<Self> {
        let local_execution_manager = Arc::new(
            aster_box_runtime::LocalExecutionManager::with_native_linux_oci_migration(
                paths.boxes_file.clone(),
                paths.home.clone(),
                config,
            )
            .await?,
        );
        Ok(Self {
            paths,
            image_cache_size: aster_box_runtime::DEFAULT_IMAGE_CACHE_SIZE,
            execution_manager: local_execution_manager.clone(),
            execution_session_manager: Some(local_execution_manager),
        })
    }

    /// Create a client with an explicit backend-neutral execution manager.
    ///
    /// This keeps lifecycle calls on the canonical facade while allowing an
    /// embedding application to inject its own manager implementation.
    pub fn with_execution_manager(
        paths: AsterBoxPaths,
        execution_manager: Arc<dyn ExecutionManager>,
    ) -> Self {
        Self {
            paths,
            image_cache_size: aster_box_runtime::DEFAULT_IMAGE_CACHE_SIZE,
            execution_manager,
            execution_session_manager: None,
        }
    }

    /// Create a client with explicit typed lifecycle and session managers.
    ///
    /// This is useful for embedding the local [`crate::Sandbox`] facade in
    /// another local process without selecting backends by string.
    pub fn with_execution_services(
        paths: AsterBoxPaths,
        execution_manager: Arc<dyn ExecutionManager>,
        execution_session_manager: Arc<dyn ExecutionSessionManager>,
    ) -> Self {
        Self {
            paths,
            image_cache_size: aster_box_runtime::DEFAULT_IMAGE_CACHE_SIZE,
            execution_manager,
            execution_session_manager: Some(execution_session_manager),
        }
    }

    /// Override the image cache size used when opening the runtime image store.
    pub fn with_image_cache_size(mut self, image_cache_size: u64) -> Self {
        self.image_cache_size = image_cache_size;
        self
    }

    /// Return the state paths used by this client.
    pub fn paths(&self) -> &AsterBoxPaths {
        &self.paths
    }

    /// Collect local runtime diagnostics without spawning the CLI.
    pub fn runtime_diagnostics(&self) -> RuntimeDiagnostics {
        RuntimeDiagnostics::collect(&self.paths)
    }

    /// Collect local runtime disk usage without spawning the CLI.
    pub fn runtime_disk_usage(&self) -> Result<RuntimeDiskUsage> {
        RuntimeDiskUsage::collect(&self.paths)
    }
}

impl Default for AsterBoxClient {
    fn default() -> Self {
        Self::with_paths(AsterBoxPaths::default())
    }
}
