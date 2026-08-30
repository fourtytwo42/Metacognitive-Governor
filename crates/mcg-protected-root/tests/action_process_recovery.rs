use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use mcg_action_runner::{
    ACTION_RUNNER_CONTENT_MAX_BYTES, ActionRunnerClient, GovernedFilesystemAuthority,
    GovernedFilesystemOperation, governed_filesystem_authority_binding,
    governed_filesystem_operation_payload,
};
use mcg_config::AppConfig;
use mcg_contracts::{
    ActionEnvelope, ActionExecution, ActionExecutionId, ActionExecutionState, ActionId, AttemptId,
    ConfigurationId, ContentHash, ContractClause, CorrelationId, EffectClass, PolicyBinding,
    RetryPolicy, RuntimeActorId, SourceRef, StableTypeId, TaskContract, TaskContractId,
    TaskContractStatus, VersionedPayload, VersionedRef, WorkflowAttempt, WorkflowId, WorkflowState,
};
use mcg_protected_root::{execute_authorized_filesystem_action, open, reconcile_filesystem_action};
use mcg_runtime::AppRuntime;
use mcg_storage::{SqliteEventStore, task_contract_content_hash};
use sha2::{Digest as _, Sha256};

fn content_hash(label: &str) -> ContentHash {
    VersionedPayload::canonical_v1(
        StableTypeId::parse("test.root_process_recovery_ref.v1").unwrap(),
        &serde_json::json!({"label": label}),
    )
    .unwrap()
    .content_hash("TestRootProcessRecoveryRef")
    .unwrap()
}

fn policy(label: &str) -> PolicyBinding {
    let payload = VersionedPayload::canonical_v1(
        StableTypeId::parse("test.root_process_recovery_policy.v1").unwrap(),
        &serde_json::json!({"policy": label}),
    )
    .unwrap();
    PolicyBinding {
        content_hash: payload
            .content_hash("TestRootProcessRecoveryPolicy")
            .unwrap(),
        payload,
    }
}

fn workflow(at: chrono::DateTime<Utc>) -> (TaskContract, WorkflowAttempt) {
    let id = TaskContractId::new();
    let source = SourceRef {
        source_type: StableTypeId::parse("test.root_process_recovery_intent.v1").unwrap(),
        locator: "test:root-process-recovery".into(),
        content_hash: content_hash("intent"),
    };
    let clause = ContractClause {
        clause_id: "recover".into(),
        statement: "reconcile uncertain child effects after Root restart".into(),
        source_refs: vec![source.clone()],
    };
    let contract = TaskContract {
        id,
        version: 1,
        previous_version: None,
        user_intent_ref: source,
        objective: "prove real child recovery".into(),
        non_goals: vec![],
        requirements: vec![clause.clone()],
        constraints: vec![],
        assumptions: vec![],
        acceptance_criteria: vec![clause],
        dependencies: vec![],
        permitted_side_effects: policy("filesystem"),
        authority: policy("root"),
        environment: policy("test"),
        risk: policy("bounded"),
        budgets: policy("bounded"),
        rollback: policy("reconcile"),
        status: TaskContractStatus::Active,
        created_at: at,
    };
    let attempt = WorkflowAttempt {
        workflow_id: WorkflowId::new(),
        attempt_id: AttemptId::new(),
        task_contract: VersionedRef {
            id,
            version: 1,
            content_hash: task_contract_content_hash(&contract).unwrap(),
        },
        parent_attempt_id: None,
        attempt_number: 1,
        state: WorkflowState::Ready,
        revision: 1,
        lease_owner: None,
        lease_generation: 0,
        lease_expires_at: None,
        heartbeat_at: None,
        checkpoint: None,
        environment_snapshot: content_hash("environment"),
        configuration: VersionedRef {
            id: ConfigurationId::new(),
            version: 1,
            content_hash: content_hash("configuration"),
        },
        idempotency_key: "root-process-recovery".into(),
        retry_policy: RetryPolicy {
            max_attempts: 2,
            initial_backoff_ms: 10,
            maximum_backoff_ms: 100,
        },
        compensation_policy: policy("reconcile"),
        created_at: at,
        updated_at: at,
    };
    (contract, attempt)
}

fn runner(root: &Path, timeout: Duration) -> ActionRunnerClient {
    let directory = root.join("runner");
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join(if cfg!(windows) {
        "mcg-action-runner.exe"
    } else {
        "mcg-action-runner"
    });
    std::fs::copy(
        env!("CARGO_BIN_EXE_mcg-protected-root-action-fault-fixture"),
        &path,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
    }
    ActionRunnerClient::with_executable(path, timeout).unwrap()
}

fn authorized_fixture() -> (
    tempfile::TempDir,
    AppConfig,
    Arc<AppRuntime>,
    ActionExecution,
    chrono::DateTime<Utc>,
) {
    let temporary = tempfile::tempdir().unwrap();
    let config = AppConfig::default().with_data_directory(temporary.path());
    let now = Utc::now();
    let (contract, attempt) = workflow(now);
    let actor = RuntimeActorId::new();
    let running = {
        let mut store = SqliteEventStore::open(config.database_path()).unwrap();
        store
            .create_workflow(&contract, &attempt, CorrelationId::new(), now)
            .unwrap();
        store
            .claim_workflow_attempt(
                attempt.attempt_id,
                1,
                0,
                actor,
                now + chrono::Duration::minutes(10),
                CorrelationId::new(),
                "root-process-recovery-claim",
                now,
            )
            .unwrap()
    };
    let runtime = open(config.clone()).unwrap();
    let capability_root = temporary.path().join("capability-root");
    std::fs::create_dir(&capability_root).unwrap();
    let action_id = ActionId::new();
    let (operation, operation_hash) =
        governed_filesystem_operation_payload(&GovernedFilesystemOperation::CreateFile {
            relative_path: "authorized.txt".into(),
            content_base64: "YXV0aG9yaXplZA==".into(),
        })
        .unwrap();
    let (compensation, _) =
        governed_filesystem_operation_payload(&GovernedFilesystemOperation::RemoveCreatedFile {
            relative_path: "authorized.txt".into(),
            expected_sha256: format!("{:x}", Sha256::digest(b"authorized")),
        })
        .unwrap();
    let (reconciler, _) =
        governed_filesystem_operation_payload(&GovernedFilesystemOperation::InspectFile {
            relative_path: "authorized.txt".into(),
            max_bytes: ACTION_RUNNER_CONTENT_MAX_BYTES,
        })
        .unwrap();
    let authority = governed_filesystem_authority_binding(&GovernedFilesystemAuthority {
        action_id,
        actor,
        capability_root,
        operation_hash: operation_hash.clone(),
        effect_class: EffectClass::Reversible,
        expires_at: now + chrono::Duration::minutes(5),
    })
    .unwrap();
    let envelope = ActionEnvelope {
        id: action_id,
        workflow_id: running.workflow_id,
        attempt_id: running.attempt_id,
        task_contract: VersionedRef {
            id: contract.id,
            version: contract.version,
            content_hash: task_contract_content_hash(&contract).unwrap(),
        },
        actor,
        origin: None,
        authority,
        elevated_authority: None,
        expected_versions: vec![],
        operation,
        operation_hash,
        effect_class: EffectClass::Reversible,
        preconditions: vec![],
        budget_reservation: policy("action-budget"),
        idempotency_key: "root-process-recovery-action".into(),
        compensation: Some(compensation),
        compensates_action: None,
        reconciler: Some(reconciler),
        reconciliation_deadline: Some(now + chrono::Duration::minutes(5)),
        created_at: now,
    };
    let execution = ActionExecution {
        id: ActionExecutionId::new(),
        action_id,
        attempt_number: 1,
        state: ActionExecutionState::Authorized,
        revision: 1,
        lease_generation: 0,
        executor: None,
        outcome: None,
        created_at: now,
        updated_at: now,
    };
    runtime
        .authorize_canonical_action(
            &envelope,
            &execution,
            running.revision,
            running.lease_generation,
            CorrelationId::new(),
            now,
        )
        .unwrap();
    (temporary, config, runtime, execution, now)
}

#[tokio::test]
async fn real_child_fault_is_durable_and_reconciles_after_root_restart() {
    for (mode, effect_exists, final_state) in [
        (
            "exit-before-effect",
            false,
            ActionExecutionState::NoEffectConfirmed,
        ),
        (
            "effect-then-exit",
            true,
            ActionExecutionState::EffectConfirmed,
        ),
        (
            "timeout-before-effect",
            false,
            ActionExecutionState::NoEffectConfirmed,
        ),
        (
            "effect-then-timeout",
            true,
            ActionExecutionState::EffectConfirmed,
        ),
    ] {
        let (temporary, config, runtime, execution, now) = authorized_fixture();
        let capability_root = temporary.path().join("capability-root");
        std::fs::write(capability_root.join("fault-mode.txt"), mode).unwrap();
        let client = runner(
            temporary.path(),
            if mode.contains("timeout") {
                Duration::from_millis(100)
            } else {
                Duration::from_secs(2)
            },
        );
        let receipt = execute_authorized_filesystem_action(
            &runtime,
            &client,
            execution.id,
            execution.revision,
            execution.lease_generation,
            RuntimeActorId::new(),
            now,
        )
        .await
        .unwrap();
        assert_eq!(
            receipt.execution.state,
            ActionExecutionState::OutcomeUnknown
        );
        assert_eq!(
            capability_root.join("authorized.txt").exists(),
            effect_exists
        );
        let job = receipt.reconciliation_job.unwrap();
        drop(runtime);

        let reopened = open(config).unwrap();
        let (_, durable) = reopened
            .canonical_action_for_execution(execution.id)
            .unwrap()
            .unwrap();
        assert_eq!(durable.state, ActionExecutionState::OutcomeUnknown);
        std::fs::write(capability_root.join("fault-mode.txt"), "inspect").unwrap();
        let reconcile_at = std::cmp::max(Utc::now(), job.next_attempt_at);
        let reconciled = reconcile_filesystem_action(
            &reopened,
            &client,
            job.id,
            job.revision,
            job.lease_generation,
            durable.revision,
            durable.lease_generation,
            RuntimeActorId::new(),
            reconcile_at,
        )
        .await
        .unwrap();
        assert_eq!(reconciled.resolution.execution.state, final_state);
        assert_eq!(
            capability_root.join("authorized.txt").exists(),
            effect_exists
        );
    }
}
