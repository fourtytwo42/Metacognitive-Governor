use std::sync::Arc;

use mcg_api::{CreateTaskRequest, TaskEventsResponse, TaskMutationResponse};
use mcg_config::AppConfig;
use mcg_domain::TaskEnvelope;
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
