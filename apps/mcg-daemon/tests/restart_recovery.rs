use std::sync::Arc;

use chrono::{Duration, Utc};
use mcg_api::{CreateTaskRequest, TaskEventsResponse, TaskMutationResponse};
use mcg_config::AppConfig;
use mcg_domain::{
    ProjectId, RiskLevel, SCHEDULE_SCHEMA_VERSION, ScheduleDefinition, ScheduleId,
    ScheduleOccurrenceState, ScheduleState, ScheduleTrigger, TaskBudget, TaskEnvelope,
};
use mcg_runtime::AppRuntime;
use tokio::{net::TcpListener, task::JoinHandle};

async fn start_server(runtime: Arc<AppRuntime>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let server = tokio::spawn(async move {
        mcg_api::serve_listener(listener, runtime, std::future::pending())
            .await
            .expect("server should run");
    });
    (format!("http://{address}"), server)
}

fn restart_schedule(now: chrono::DateTime<Utc>) -> ScheduleDefinition {
    ScheduleDefinition {
        schema_version: SCHEDULE_SCHEMA_VERSION,
        id: ScheduleId::new(),
        definition_hash: String::new(),
        name: "restart recovery".into(),
        project_id: ProjectId::new(),
        goal: "recover exactly once".into(),
        constraints: Vec::new(),
        success_conditions: Vec::new(),
        risk: RiskLevel::Medium,
        budget: TaskBudget {
            max_tokens: Some(4_096),
            max_cost_usd: Some(1.0),
            max_duration_seconds: Some(300),
        },
        trigger: ScheduleTrigger::Interval { seconds: 60 },
        state: ScheduleState::Enabled,
        next_run_at: Some(now),
        target_path: None,
        context_token_budget: 4_096,
        requested_tools: Vec::new(),
        estimated_output_tokens: 1_024,
        max_model_calls: 2,
        watch_fingerprint: None,
        last_watch_at: None,
        last_event_sequence: 0,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn in_flight_schedule_reclaims_same_task_immediately_after_restart() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let config = AppConfig::default().with_data_directory(directory.path());
    let now = Utc::now();
    let schedule = restart_schedule(now);
    let first = AppRuntime::open(config.clone()).expect("open first runtime");
    first.create_schedule(&schedule).expect("persist schedule");
    let original = first
        .claim_due_schedule(now, "first-daemon", 300)
        .expect("claim occurrence")
        .expect("due occurrence");
    drop(first);

    let recovered = AppRuntime::open(config).expect("recover runtime");
    assert_eq!(recovered.status().recovered_schedule_occurrences, 1);
    let replay = recovered
        .claim_due_schedule(now + Duration::seconds(1), "second-daemon", 300)
        .expect("reclaim occurrence")
        .expect("recovered occurrence");
    assert_eq!(replay.task.id, original.task.id);
    assert_eq!(replay.occurrence.id, original.occurrence.id);
    assert_eq!(replay.occurrence.attempt, 2);
    assert_eq!(replay.occurrence.state, ScheduleOccurrenceState::Recovering);
    assert!(replay.task_created_event.is_none());
}

#[tokio::test]
async fn task_history_and_projection_survive_abrupt_daemon_restart() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let config = AppConfig::default().with_data_directory(directory.path());
    let first_runtime = Arc::new(AppRuntime::open(config.clone()).expect("runtime should open"));
    let first_token = first_runtime.auth().token_for_local_client().to_owned();
    let (first_host, first_server) = start_server(first_runtime.clone()).await;
    let client = reqwest::Client::new();

    let created: TaskMutationResponse = client
        .post(format!("{first_host}/api/v1/tasks"))
        .bearer_auth(&first_token)
        .json(&CreateTaskRequest {
            goal: "survive abrupt daemon restart".to_owned(),
            project_id: None,
            thread_id: None,
            constraints: vec!["preserve ordered history".to_owned()],
            success_conditions: vec!["projection equals committed event".to_owned()],
            risk: None,
            budget: None,
        })
        .send()
        .await
        .expect("create request should complete")
        .error_for_status()
        .expect("create should succeed")
        .json()
        .await
        .expect("create response should decode");

    first_server.abort();
    let _ = first_server.await;
    drop(first_runtime);

    let second_runtime = Arc::new(AppRuntime::open(config).expect("runtime should recover"));
    assert_eq!(second_runtime.auth().token_for_local_client(), first_token);
    assert_eq!(second_runtime.status().recovered_tasks, 1);
    assert_eq!(second_runtime.status().recovered_events, 1);
    let (second_host, second_server) = start_server(second_runtime).await;

    let recovered: TaskEnvelope = client
        .get(format!("{second_host}/api/v1/tasks/{}", created.task.id))
        .bearer_auth(&first_token)
        .send()
        .await
        .expect("get request should complete")
        .error_for_status()
        .expect("get should succeed")
        .json()
        .await
        .expect("task response should decode");
    let history: TaskEventsResponse = client
        .get(format!(
            "{second_host}/api/v1/tasks/{}/events",
            created.task.id
        ))
        .bearer_auth(&first_token)
        .send()
        .await
        .expect("history request should complete")
        .error_for_status()
        .expect("history should succeed")
        .json()
        .await
        .expect("history response should decode");

    assert_eq!(recovered, created.task);
    assert_eq!(history.events, vec![created.event]);
    second_server.abort();
    let _ = second_server.await;
}
