//! Aster Code Core Library
//!
//! Harness-driven runtime for coding agents.
//!
//! `Agent` and `AgentSession` are the primary 2.0 API. Lower-level session
//! runtime state is internal; persistence data flows through `store::SessionData`.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use aster_core::{Agent, AgentEvent};
//!
//! # async fn run() -> anyhow::Result<()> {
//! // From an ACL-compatible config file path (.acl)
//! let agent = Agent::new("agent.acl").await?;
//!
//! // Create a workspace-bound session
//! let session = agent.session_async("/my-project", None).await?;
//!
//! // Non-streaming
//! let result = session.send("What files handle auth?", None).await?;
//! println!("{}", result.text);
//!
//! // Streaming (AgentEvent is #[non_exhaustive])
//! let (mut rx, _handle) = session.stream("Refactor auth", None).await?;
//! while let Some(event) = rx.recv().await {
//!     match event {
//!         AgentEvent::TextDelta { text } => print!("{text}"),
//!         AgentEvent::End { .. } => break,
//!         _ => {} // required: #[non_exhaustive]
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Disposable Workers
//!
//! ```rust,no_run
//! use aster_core::{Agent, SessionOptions, WorkerAgentSpec};
//!
//! # async fn run() -> anyhow::Result<()> {
//! let agent = Agent::new("agent.acl").await?;
//! let frontend = WorkerAgentSpec::implementer(
//!     "frontend-cow",
//!     "Small verified frontend fixes",
//! )
//! .with_model_ref("openai/gpt-4o")
//! .with_max_steps(24);
//!
//! let session = agent.session_async(
//!     "/my-project",
//!     Some(SessionOptions::new().with_worker_agent(frontend)),
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! Agent (config-driven facade)
//!   +-- AgentSession (workspace-bound execution API)
//!       +-- internal turn runner
//!       +-- ContextAssembler / ContextProvider
//!       +-- ToolSelector
//!       +-- ToolExecutor
//!       +-- ProgramExecutor (PTC)
//!       +-- SkillRegistry
//!       +-- Permission / confirmation
//!       +-- Trace / artifacts / verification evidence
//!
//! Advanced infrastructure:
//!   +-- optional lane queues for explicit external/hybrid dispatch
//! ```

pub(crate) mod agent;
pub(crate) mod agent_api;
pub mod agent_protocol;
pub mod agent_protocol_harness;
pub mod agent_protocol_host;
pub mod ask_user;
pub(crate) mod bounded_io;
pub mod budget;
pub mod capability;
pub(crate) mod child_run;
pub mod code_intelligence;
pub mod commands;
pub(crate) mod compaction;
pub mod package_context;
#[doc(hidden)]
pub use compaction::{compaction_system_prompt, render_message_for_summary};
pub(crate) mod compaction_carry;
pub mod completion_attestor;
pub mod config;
pub mod content_digest;
pub mod context;
pub mod core_event_log;
pub mod core_identity;
/// Real-model deep-case lineage and op-coverage compile behind `deep-live`.
#[cfg(test)]
mod deep_e2e_live;
pub mod directory_projection;
pub mod durable_memory;
#[cfg(feature = "aster-vec-fts")]
pub mod durable_memory_vec;
pub mod effect_isolation;
pub mod embedding;
pub mod error;
pub mod event_protocol;
pub mod execution_identity;
pub mod external_observation;
pub mod fact_control;
pub(crate) mod git;
pub mod goal_control;
pub mod harness_evidence;
pub mod harness_loop;
pub mod hitl;
pub mod hooks;
pub mod host_env;
pub(crate) mod language;
pub mod llm;
pub mod loop_checkpoint;
pub mod mcp;
pub mod memory;
pub mod meta_harness;
#[cfg(feature = "headless-search")]
pub mod moli_runtime;
#[cfg(all(test, feature = "deep-live"))]
mod op_coverage;
pub mod orchestration;
pub(crate) mod ordered_parallel;
pub mod outcome_memory;
pub mod path_instructions;
pub mod permissions;
pub mod planning;
pub(crate) mod porcelain;
pub mod program;
pub(crate) mod prompts;
pub mod queue;
mod rank_fusion;
pub mod read_only_verifier;
pub mod release;
pub mod retention;
pub(crate) mod retry;
pub mod rl_trajectory;
pub mod run;
pub use run::schedule;
pub mod run_control;
pub(crate) mod safety_gate;
pub mod sandbox;
pub mod sdk_capabilities;
#[cfg(feature = "headless-search")]
pub mod search_runtime;
pub mod secure_state;
pub mod security;
pub mod session_checkpoint;
pub mod session_folder;
pub(crate) mod session_lane_queue;
pub mod session_review;
pub mod shell_session;
pub mod skills;
pub(crate) mod sse;
pub(crate) mod standing_context;
pub mod state_root;
pub mod store;
pub mod subagent;
pub mod subagent_task_tracker;
pub(crate) mod surface_decision;
pub mod task_scheduler;
pub mod telemetry;
#[cfg(test)]
pub(crate) mod test_mutex_poison;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod text;
pub(crate) mod tool_confirmation;
pub mod tool_name;
pub mod tools;
pub mod trace;
pub mod transcript;
pub mod use_harness;
pub mod use_runtime_tasks;
pub mod verification;
mod work_policy;
pub mod workspace;

// Re-export key types at crate root for ergonomic usage. A
// `#[doc(hidden)]` re-export is a deprecated alias: import it from its
// module path. Aliases are removed no earlier than 10.0 and may only shrink
// (`tests/crate_root_surface.rs`).
#[doc(hidden)]
pub use agent::{context_window_for_model, documented_context_tokens, AgentExecutionFailure};
pub use agent::{hostable_context_window, AgentEvent, AgentResult, ModelMiddlewareHealthSnapshot};
pub use agent_api::{
    resolve_session_llm_client, validate_output_schema, Agent, AgentRunSpawn, AgentRunStream,
    AgentSession, FollowUpSuggestions, GoalTickAdmission, GoalTickStream, ReadFileOptions,
    SessionOptions, ToolCallResult,
};
#[doc(hidden)]
pub use agent_api::{HeavyToolResult, ProjectedUiHandle, SessionBuilder, SessionContextReport};
pub use agent_protocol::AgentProtocolModelPinV1;
#[doc(hidden)]
pub use agent_protocol::{
    AgentProtocolChangeSetRequestV1, AgentProtocolChangeSetV1, AgentProtocolCommandActionV1,
    AgentProtocolCommandReceiptV1, AgentProtocolCommandV1, AgentProtocolError,
    AgentProtocolEventPageRequestV1, AgentProtocolEventPageV1, AgentProtocolEventRecordV1,
    AgentProtocolRunCancelV1, AgentProtocolRunIdentityV1, AgentProtocolRunRecoverExactV1,
    AgentProtocolRunRecoverV1, AgentProtocolRunStartV1, AgentProtocolRunStateV1,
    AGENT_PROTOCOL_CHANGE_SET_ENCODING_V1, AGENT_PROTOCOL_CHANGE_SET_FORMAT_V1,
    AGENT_PROTOCOL_CHANGE_SET_HTTP_PATH_V1, AGENT_PROTOCOL_COMMAND_HTTP_PATH_V1,
    AGENT_PROTOCOL_EVENT_PAGE_HTTP_PATH_V1, AGENT_PROTOCOL_MAX_CHANGE_SET_BYTES,
    AGENT_PROTOCOL_MAX_CHANGE_SET_RESPONSE_BYTES, AGENT_PROTOCOL_MAX_EVENTS_PER_PAGE,
    AGENT_PROTOCOL_MAX_EVENT_METADATA_BYTES, AGENT_PROTOCOL_MAX_EVENT_PAGE_BYTES,
    AGENT_PROTOCOL_MAX_EVENT_PAYLOAD_BYTES, AGENT_PROTOCOL_MAX_EVENT_RECORD_BYTES,
    AGENT_PROTOCOL_MAX_EVENT_TYPE_BYTES, AGENT_PROTOCOL_MAX_ID_BYTES,
    AGENT_PROTOCOL_MAX_PROMPT_BYTES, AGENT_PROTOCOL_MAX_REASON_BYTES, AGENT_PROTOCOL_V1,
};
#[doc(hidden)]
pub use agent_protocol_harness::{
    force_invalid_recovery_binding_for_test, AgentProtocolCheckpointRecoveryError,
    AgentProtocolHarness, AgentProtocolHarnessError, AGENT_PROTOCOL_HARNESS_MAX_SESSIONS,
};
#[doc(hidden)]
pub use agent_protocol_host::{
    AgentProtocolExactRecoveryError, AgentProtocolHost, AgentProtocolHostError,
};
#[doc(hidden)]
pub use code_intelligence::{
    CodeDiagnostic, CodeDiagnosticSeverity, CodeIntelligenceCapabilities, CodeIntelligenceError,
    CodeIntelligenceLanguageStatus, CodeIntelligenceResult, CodeIntelligenceState,
    CodeIntelligenceStatus, CodeLocation, CodePosition, CodeQueryResult, CodeRange, CodeSymbolKind,
    DocumentRevision, DocumentSnapshot, DocumentSymbol, LanguageId, LocalCodeIntelligence,
    NavigationKind, SymbolInformation, WorkspaceCodeIntelligence,
};
#[doc(hidden)]
pub use completion_attestor::{merge_attested_report, CompletionAttestor, MutatedPathRecord};
#[doc(hidden)]
pub use config::OsConfig;
pub use config::{
    AutoDelegationConfig, CodeConfig, ModelConfig, ModelCost, ModelLimit, ModelModalities,
    ProviderConfig, ProviderProtocol,
};
pub use content_digest::{digest_bytes, digest_json, validate_digest};
pub use core_event_log::CoreEventLog;
#[doc(hidden)]
pub use core_event_log::{
    CoreEventLogError, CoreLogAppendOutcomeV1, CoreLogEntryV1, CoreLogPageV1,
    CoreLogVerificationV1, CORE_LOG_ENTRY_DIGEST_DOMAIN_V1, CORE_LOG_ENTRY_SCHEMA_V1,
    CORE_LOG_GENESIS_PREVIOUS_DIGEST,
};
#[doc(hidden)]
pub use core_identity::{
    ArtifactRef, CoreEventIdentity, CoreIdentity, CoreIdentityError, EvidenceCursor, LogicalClock,
    ManualLogicalClock, SystemLogicalClock, CORE_EVENT_IDENTITY_DIGEST_DOMAIN_V1,
    CORE_EVENT_IDENTITY_SCHEMA_V1, CORE_EVENT_PAYLOAD_DIGEST_DOMAIN_V1,
    CORE_IDENTITY_MAX_ARTIFACT_BYTES, CORE_IDENTITY_MAX_EVENT_TYPE_BYTES,
    CORE_IDENTITY_MAX_ID_BYTES, CORE_IDENTITY_MAX_MEDIA_TYPE_BYTES,
    CORE_IDENTITY_MAX_PAYLOAD_BYTES, CORE_IDENTITY_SCHEMA_V1,
};
pub use core_identity::{CapabilityStamp, OperationId, SourceRevision};
#[doc(hidden)]
pub use durable_memory::{
    DurableMemoryActivation, DurableMemoryBindingV1, DurableMemoryMode, DurableMemoryRecallChannel,
    DurableMemoryRecallHit, DurableMemoryRecallPolicy, DurableMemoryRecallPreview,
    DurableMemorySemanticBindingV1, DurableMemorySemanticError, DurableMemorySemanticRecall,
    DurableMemorySemanticRecallPolicy, DurableMemorySemanticRefreshCheckpoint,
    DurableMemorySemanticRefreshReceipt, DurableMemorySession, DurableMemoryUse,
    DURABLE_MEMORY_BINDING_SCHEMA_VERSION, DURABLE_MEMORY_CONTEXT_ID_PROFILE_V1,
    DURABLE_MEMORY_CONTEXT_ID_PROFILE_V2, DURABLE_MEMORY_HYBRID_BINDING_SCHEMA_VERSION,
    DURABLE_MEMORY_RETRIEVAL_PROFILE_V1, DURABLE_MEMORY_SEMANTIC_BINDING_SCHEMA_V1,
    DURABLE_MEMORY_SEMANTIC_FUSION_PROFILE_V1, DURABLE_MEMORY_SEMANTIC_LINEAR_FUSION_PROFILE_V1,
    DURABLE_MEMORY_SEMANTIC_REFRESH_CHECKPOINT_SCHEMA_V1,
    DURABLE_MEMORY_SEMANTIC_REFRESH_PROFILE_V1,
};
pub use embedding::EmbeddingProvider;
#[doc(hidden)]
pub use embedding::{
    EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingError, EmbeddingExecution,
    EmbeddingExecutor, EmbeddingExecutorConfig, EmbeddingFailureKind, EmbeddingInput,
    EmbeddingNormalization, EmbeddingProviderDescriptor, EmbeddingProviderError, EmbeddingResult,
    EmbeddingVector,
};
pub use error::CodeError;
#[doc(hidden)]
pub use error::Result;
#[doc(hidden)]
pub use error::SessionBuildResource;
#[doc(hidden)]
pub use event_protocol::AgentEventTypeV1;
pub use event_protocol::{
    run_event_envelope_v1, AgentEventProjectionV1, EventEnvelopeV1, EventProtocolError,
    AGENT_EVENT_TYPES_V1, EVENT_ENVELOPE_V1_VERSION,
};
#[doc(hidden)]
pub use harness_evidence::{
    HarnessEvidenceError, ModelInputKindV1, ModelInputSnapshotV1, ModelPresentationApplicationV1,
    ModelPresentationSnapshotV1, ModelUsageSnapshotV1, RunCapabilitySnapshotV1,
    RunPolicyCeilingSnapshotV1, ToolRequestOriginV1, ToolRequestSnapshotV1,
    ToolResultContextUsageV1, WorkspaceCapabilitySnapshotV1,
    WorkspaceRetrievalCapabilitySnapshotV1, MODEL_INPUT_SNAPSHOT_V1_SCHEMA,
    MODEL_PRESENTATION_SNAPSHOT_V1_SCHEMA, MODEL_USAGE_SNAPSHOT_V1_SCHEMA,
    RUN_CAPABILITY_SNAPSHOT_V1_SCHEMA, TOOL_REQUEST_SNAPSHOT_V1_SCHEMA,
};
#[doc(hidden)]
pub use llm::{
    clear_http_metrics_callback, set_http_metrics_callback, strip_leaked_tool_protocol,
    AnthropicClient, ContentBlock, HttpMetricsCallback, HttpMetricsRecord, ImageSource,
    ModelGenerationAdmission, ModelGenerationAdmissionError, ModelGenerationConcurrency,
    ModelGenerationPermit, ModelGenerationPool, ModelGenerationPoolError, OpenAiClient,
};
pub use llm::{
    Attachment, LlmClient, LlmResponse, Message, ModelGenerationPoolHealthSnapshot, TokenUsage,
};
#[doc(hidden)]
pub use meta_harness::{
    admit_component_tree, admit_default_graph, admit_from_compose, admit_spec_graph,
    parse_harness_components, parse_harness_part, parse_harness_parts, HarnessComponentRef,
    HostHarnessAssembler, HostHarnessRegistry, KernelPolicy, INTENT_STAMP_MARKER,
};
pub use meta_harness::{
    admit_from_compose_with_registry, host_component_id, parse_harness_component,
    BuiltinHostHarnessRegistry, HarnessComposeOptions, HarnessConfig, HarnessPartId,
};
#[cfg(feature = "headless-search")]
#[doc(hidden)]
pub use moli_runtime::packaged_moli;
#[cfg(feature = "headless-search")]
pub use moli_runtime::{
    default_moli_version, ensure_moli, moli_runtime_info, MoliRuntimeInfo,
    MOLI_RUNTIME_INFO_SCHEMA_V1,
};
#[doc(hidden)]
pub use orchestration::{
    execute_loop, execute_pipeline, execute_steps_parallel, workflow_step_execution_identity,
    workflow_step_result_receipt, AgentExecutor, BudgetSnapshot, LoopDecision, PipelineStage,
    StepOutcome, Workflow, WorkflowBudget, WorkflowBuilder, WorkflowCheckpoint, WorkflowEvent,
    WorkflowStepRecord, WORKFLOW_CHECKPOINT_SCHEMA_VERSION,
};
pub use orchestration::{execute_steps_parallel_resumable, AgentStepSpec};
#[doc(hidden)]
pub use package_context::{
    PackageBindingV1, PackageContextDocumentV1, PackageContextError, PackageContextLimits,
    PackageContextProvider, PackageContextRequestV1, PackageContextResponseV1,
    PackageContextResult, PackageContextSession, PackageKnowledgeBindingV1,
    PackageKnowledgeCitationV1, OKF_KNOWLEDGE_CITATION_SCHEMA, OKF_KNOWLEDGE_READ_REQUEST_SCHEMA,
    OKF_KNOWLEDGE_SEARCH_REQUEST_SCHEMA, PACKAGE_BINDING_SCHEMA,
    PACKAGE_CONTEXT_REQUEST_DIGEST_DOMAIN, PACKAGE_CONTEXT_REQUEST_SCHEMA,
    PACKAGE_CONTEXT_RESPONSE_SCHEMA, PACKAGE_KNOWLEDGE_BINDING_SCHEMA,
};
#[doc(hidden)]
pub use prompts::{
    infer_user_reply_language, output_language_contract, resolve_product_output_language,
    DetectionConfidence,
};
pub use prompts::{AgentStyle, PlanningMode, SystemPromptSlots};
#[doc(hidden)]
pub use rl_trajectory::RlTrajectoryRecorder;
pub use rl_trajectory::{RlTrajectoryConfig, RlTrajectoryMode};
#[doc(hidden)]
pub use run::{
    ActiveToolSnapshot, InMemoryRunStore, RunRecord, RunReservation, RunSnapshot, RunStatus,
    RunWorkspaceChangeSet, RunWorkspaceChangeSetError,
};
pub use run::{RunEventRecord, RunHandle};
pub use run_control::{InterruptRequest, SteerRequest};
#[doc(hidden)]
pub use run_control::{
    RunControlCommand, RunControlError, RunControlErrorInfo, RunControlOperation,
    RunControlReceipt, RunControlReceiptState, RunControlRequest, RunControlSnapshot,
    RUN_CONTROL_MAX_ID_BYTES, RUN_CONTROL_MAX_INPUT_BYTES, RUN_CONTROL_MAX_QUEUE,
    RUN_CONTROL_MAX_REASON_BYTES, RUN_CONTROL_MAX_SEEN_REQUESTS, RUN_CONTROL_RECEIPT_SCHEMA_V1,
    RUN_CONTROL_REQUEST_SCHEMA_V1,
};
#[doc(hidden)]
pub use sdk_capabilities::{sdk_baseline_capabilities, SDK_CAPABILITIES_SCHEMA_V2};
pub use sdk_capabilities::{
    sdk_capabilities, sdk_capabilities_schema, CapabilityTier, SdkCapability,
    SDK_CAPABILITIES_SCHEMA_V1,
};
pub use session_checkpoint::{
    SdkSessionCheckpointExportV1, SessionCheckpointExportSink, SessionCheckpointExportV1,
};
#[doc(hidden)]
pub use session_checkpoint::{
    SessionCheckpointDescriptorV1, SessionCheckpointError, SessionCheckpointPayloadV1,
    SessionLogicalResumeEvidenceV1, SessionSnapshotEvidenceV1,
    SESSION_CHECKPOINT_DESCRIPTOR_SCHEMA_V1, SESSION_CHECKPOINT_ENCODING_V1,
    SESSION_CHECKPOINT_FORMAT_V1, SESSION_CHECKPOINT_LOGICAL_RESUME_SEMANTICS_V1,
    SESSION_CHECKPOINT_MAX_CONTENT_BYTES, SESSION_CHECKPOINT_MEDIA_TYPE_V1,
    SESSION_CHECKPOINT_PAYLOAD_SCHEMA_V1, SESSION_LOGICAL_RESUME_EVIDENCE_SCHEMA_V1,
    SESSION_SNAPSHOT_EVIDENCE_SCHEMA_V1,
};
pub use session_review::SessionReviewFindingV1;
#[doc(hidden)]
pub use session_review::{
    SessionReviewAnchorV1, SessionReviewSeverityV1, SessionReviewStatusV1, SessionReviewStoreV1,
    SCENARIO_REPLY_TRANSCRIPT, SESSION_REVIEW_FINDING_SCHEMA_V1, SESSION_REVIEW_STORE_SCHEMA_V1,
};
pub use subagent::{
    AgentDefinition, AgentRegistry, ConfirmationInheritance, WorkerAgentKind, WorkerAgentSpec,
};
#[doc(hidden)]
pub use subagent::{CattleAgentKind, CattleAgentSpec};
#[doc(hidden)]
pub use subagent_task_tracker::{
    InMemorySubagentTaskTracker, SubagentProgressEntry, SubagentStatus, SubagentTaskSnapshot,
};
#[doc(hidden)]
pub use task_scheduler::{
    TaskLease, TaskScheduler, TaskSchedulerConfig, TaskSchedulerQuota, TaskSchedulerQuotaSnapshot,
    TASK_SCHEDULER_MAX_QUOTAS, TASK_SCHEDULER_MAX_SCOPE_BYTES,
    TASK_SCHEDULER_QUOTA_HEALTH_RETENTION,
};
pub use task_scheduler::{
    TaskPriority, TaskPriorityCounts, TaskSchedulerError, TaskSchedulerHealthSnapshot,
    TaskSchedulerQuotaHealthSnapshot, TaskSchedulerStats,
};
pub use tools::ToolErrorKind;
pub use tools::{
    ImmutableContentAdapter, ImmutableContentAdapterBindingV1, ImmutableContentAdapterSession,
    ImmutableContentError, ImmutableContentReferenceV1, ImmutableContentResult,
    ImmutableContentWriteRequestV1, SdkImmutableContentWriteRequestV1,
};
#[doc(hidden)]
pub use tools::{
    ImmutableContentDescriptorV1, ImmutableContentKindV1, ToolPresentationError,
    ToolPresentationModeV1, ToolPresentationProfileV1, IMMUTABLE_CONTENT_ADAPTER_BINDING_SCHEMA_V1,
    IMMUTABLE_CONTENT_DESCRIPTOR_SCHEMA_V1, IMMUTABLE_CONTENT_REFERENCE_SCHEMA_V1,
    TOOL_PRESENTATION_PROFILE_V1_SCHEMA, TOOL_RESULT_CONTENT_MEDIA_TYPE,
};
#[doc(hidden)]
pub use tools::{ToolCapabilities, ToolOutputKind, ToolResultTrustV1};
#[doc(hidden)]
pub use use_runtime_tasks::{
    UsePlanScope, UsePlanScopeKind, UseProjectedLifecycleIdentity, UseRuntimeTaskDispatcher,
    UseRuntimeTaskError, UseRuntimeTaskExecutionV1, UseRuntimeTaskProjectionAdapter,
    UseRuntimeTaskProjectionV1, UseRuntimeTaskRequestV1, UseRuntimeTaskResult,
    MAX_USE_RUNTIME_TASK_ARGUMENTS, MAX_USE_RUNTIME_TASK_ARGUMENT_BYTES,
    MAX_USE_RUNTIME_TASK_OUTPUT_BYTES, MAX_USE_RUNTIME_TASK_TIMEOUT_MS,
    USE_RUNTIME_TASK_REQUEST_SCHEMA, USE_RUNTIME_TASK_RESULT_SCHEMA,
};
pub use work_policy::WorkPolicyV1;
#[doc(hidden)]
pub use work_policy::{WorkPolicyError, WorkPolicyRefusal, WorkPolicyRefusalParseError};
pub use workspace::{
    ChunkCatalogLimits, ChunkingConfig, FixedWindowChunkingOptions, RecursiveChunkingOptions,
    WorkspaceChunk, WorkspaceChunkCatalog, WorkspaceChunkingError, WorkspaceChunkingStrategy,
    WorkspaceEmbeddingBatchMetrics, WorkspaceHybridChannelRank, WorkspaceHybridChannelStatus,
    WorkspaceHybridSearchHit, WorkspaceHybridSearchRequest, WorkspaceHybridSearchResult,
    WorkspaceLexicalEngine, WorkspacePath, WorkspaceRerankAlgorithm, WorkspaceRerankFallbackReason,
    WorkspaceRerankMode, WorkspaceRerankOptions, WorkspaceRerankStatus, WorkspaceRetrievalError,
    WorkspaceRetrievalOptions, WorkspaceRetrievalStatus, WorkspaceSemanticIndexLimits,
    WorkspaceSemanticSearchHit, WorkspaceSemanticSearchRequest, WorkspaceSemanticSearchResult,
    WorkspaceServices,
};
#[doc(hidden)]
pub use workspace::{
    ChunkCatalogSnapshot, CommandOutput, CommandOutputObserver, CommandOutputSummary,
    CommandRequest, CustomWorkspaceChunkingStrategy, GrepCandidateIndex, GrepCandidateSelection,
    LexicalSearchHit, LexicalSearchRequest, LexicalSearchResult, LocalWorkspaceAccessPolicy,
    LocalWorkspaceBackend, LocalWorkspaceFile, LocalWorkspaceFileStatus, LocalWorkspaceManifest,
    LocalWorkspaceManifestSnapshot, ManifestWorkspaceBackend, RecentWorkspaceFile,
    SharedGrepCandidateIndex, UnconstrainedGrepCandidateIndex, VirtualPathResolver,
    WorkspaceCapabilities, WorkspaceChunkId, WorkspaceChunkRange, WorkspaceChunkingInput,
    WorkspaceCommandRunner, WorkspaceDirEntry, WorkspaceEligibilityPolicy, WorkspaceError,
    WorkspaceFileChange, WorkspaceFileChangeKind, WorkspaceFileSystem, WorkspaceFileSystemExt,
    WorkspaceFileType, WorkspaceGit, WorkspaceGitBranch, WorkspaceGitCheckoutOutput,
    WorkspaceGitCheckoutRequest, WorkspaceGitCommit, WorkspaceGitCreateBranchRequest,
    WorkspaceGitCreateWorktreeRequest, WorkspaceGitDiffRequest, WorkspaceGitRemote,
    WorkspaceGitRemoveWorktreeRequest, WorkspaceGitStash, WorkspaceGitStashProvider,
    WorkspaceGitStashRequest, WorkspaceGitStatus, WorkspaceGitWorktree,
    WorkspaceGitWorktreeMutation, WorkspaceGitWorktreeProvider, WorkspaceGlobRequest,
    WorkspaceGlobResult, WorkspaceGrepOutcome, WorkspaceGrepRequest, WorkspaceGrepResult,
    WorkspaceHybridFallbackReason, WorkspaceIndexError, WorkspacePathResolver,
    WorkspacePersistentIndex, WorkspacePersistentIndexPhase, WorkspacePersistentIndexStatus,
    WorkspaceRef, WorkspaceResult, WorkspaceRetrievalChannel, WorkspaceRetrievalPhase,
    WorkspaceRetrievalResult, WorkspaceRetrievalRuntime, WorkspaceSearch,
    WorkspaceSemanticFallbackReason, WorkspaceServicesBuilder, WorkspaceTextRange,
    WorkspaceTextReader, WorkspaceVersionConflict, WorkspaceWriteOutcome,
};
#[cfg(feature = "grep-trigram")]
#[doc(hidden)]
pub use workspace::{
    TrigramGrepCandidateIndex, TrigramGrepCandidateIndexError, AUTO_GREP_TRIGRAM_MAX_FILES,
    GREP_TRIGRAM_INDEX_RELATIVE_DIR,
};
