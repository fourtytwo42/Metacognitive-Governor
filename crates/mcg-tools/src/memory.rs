use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mcg_domain::ToolExecutionId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::{
    TOOL_SCHEMA_VERSION, ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus, ToolRequest,
    ToolRequestKind, VerificationEvidence,
};

const MAX_MEMORY_VALUE_BYTES: usize = 1_048_576;
const MAX_SEARCH_RESULTS: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub key: String,
    pub value: Value,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait MemoryRepository: Send + Sync {
    async fn search(&self, query: &str, limit: u64) -> Result<Vec<MemoryRecord>, ToolError>;
    async fn read(&self, key: &str) -> Result<Option<MemoryRecord>, ToolError>;
    async fn write(&self, record: MemoryRecord) -> Result<(), ToolError>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryMemoryRepository {
    records: Arc<RwLock<BTreeMap<String, MemoryRecord>>>,
}

#[async_trait]
impl MemoryRepository for InMemoryMemoryRepository {
    async fn search(&self, query: &str, limit: u64) -> Result<Vec<MemoryRecord>, ToolError> {
        let query = query.to_ascii_lowercase();
        let limit = usize::try_from(limit)
            .map_err(|_| ToolError::Invalid("memory search limit is too large".into()))?;
        Ok(self
            .records
            .read()
            .await
            .values()
            .filter(|record| {
                record.key.to_ascii_lowercase().contains(&query)
                    || record
                        .value
                        .to_string()
                        .to_ascii_lowercase()
                        .contains(&query)
            })
            .take(limit)
            .cloned()
            .collect())
    }

    async fn read(&self, key: &str) -> Result<Option<MemoryRecord>, ToolError> {
        Ok(self.records.read().await.get(key).cloned())
    }

    async fn write(&self, record: MemoryRecord) -> Result<(), ToolError> {
        self.records
            .write()
            .await
            .insert(record.key.clone(), record);
        Ok(())
    }
}

pub struct MemoryExecutor<R> {
    repository: R,
}

impl<R> MemoryExecutor<R>
where
    R: MemoryRepository,
{
    #[must_use]
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    async fn search(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        query: &str,
        limit: u64,
    ) -> Result<ToolOutcome, ToolError> {
        if query.trim().is_empty() {
            return Err(ToolError::Invalid("memory search query is required".into()));
        }
        if limit == 0 || limit > MAX_SEARCH_RESULTS {
            return Err(ToolError::Invalid(format!(
                "memory search limit must be between 1 and {MAX_SEARCH_RESULTS}"
            )));
        }
        let records = self.repository.search(query, limit).await?;
        encoded_outcome(
            execution_id,
            request,
            format!("found {} memory records", records.len()),
            &records,
            "memory.search_bounded",
        )
    }

    async fn read(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        key: &str,
    ) -> Result<ToolOutcome, ToolError> {
        validate_key(key)?;
        let record = self
            .repository
            .read(key)
            .await?
            .ok_or_else(|| ToolError::Execution("memory key was not found".into()))?;
        encoded_outcome(
            execution_id,
            request,
            "read memory record".into(),
            &record,
            "memory.key_observed",
        )
    }

    async fn write(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        key: &str,
        value: &Value,
    ) -> Result<ToolOutcome, ToolError> {
        validate_key(key)?;
        let encoded = serde_json::to_vec(value)
            .map_err(|error| ToolError::Invalid(format!("serialize memory value: {error}")))?;
        if encoded.len() > MAX_MEMORY_VALUE_BYTES {
            return Err(ToolError::Invalid(format!(
                "memory value exceeds {MAX_MEMORY_VALUE_BYTES} bytes"
            )));
        }
        let record = MemoryRecord {
            key: key.into(),
            value: value.clone(),
            updated_at: Utc::now(),
        };
        self.repository.write(record.clone()).await?;
        let observed = self
            .repository
            .read(key)
            .await?
            .ok_or_else(|| ToolError::Execution("memory write was not observable".into()))?;
        if observed.value != record.value {
            return Err(ToolError::Execution(
                "memory read-after-write verification failed".into(),
            ));
        }
        encoded_outcome(
            execution_id,
            request,
            "wrote and verified memory record".into(),
            &observed,
            "memory.read_after_write",
        )
    }
}

#[async_trait]
impl<R> ToolExecutor for MemoryExecutor<R>
where
    R: MemoryRepository,
{
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        match &request.kind {
            ToolRequestKind::MemorySearch { query, limit } => {
                self.search(execution_id, request, query, *limit).await
            }
            ToolRequestKind::MemoryRead { key } => self.read(execution_id, request, key).await,
            ToolRequestKind::MemoryWrite { key, value } => {
                self.write(execution_id, request, key, value).await
            }
            _ => Err(ToolError::Invalid(
                "memory executor received an unsupported tool".into(),
            )),
        }
    }
}

fn validate_key(key: &str) -> Result<(), ToolError> {
    let lower = key.to_ascii_lowercase();
    if key.is_empty()
        || key.len() > 512
        || key.chars().any(char::is_control)
        || ["credential/", "credentials/", "secret/", "secrets/"]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
    {
        return Err(ToolError::Denied(
            "memory key is invalid or reserved for secret isolation".into(),
        ));
    }
    Ok(())
}

fn encoded_outcome<T: Serialize>(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    summary: String,
    value: &T,
    method: &str,
) -> Result<ToolOutcome, ToolError> {
    let encoded = serde_json::to_string(value)
        .map_err(|error| ToolError::Execution(format!("serialize memory result: {error}")))?;
    let now = Utc::now();
    Ok(ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Succeeded,
        summary,
        exit_code: None,
        stdout: Some(encoded),
        stderr: None,
        artifacts: Vec::new(),
        verification: vec![VerificationEvidence {
            method: method.into(),
            passed: true,
            detail: "true".into(),
        }],
        started_at: now,
        finished_at: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_is_read_after_write_verified_and_searchable() {
        let executor = MemoryExecutor::new(InMemoryMemoryRepository::default());
        let write = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "remember",
            ToolRequestKind::MemoryWrite {
                key: "project/decision".into(),
                value: serde_json::json!({"decision":"use typed tools"}),
            },
        );
        executor
            .execute(ToolExecutionId::new(), &write)
            .await
            .expect("write");
        let search = ToolRequest::new(
            write.task_id,
            "search",
            ToolRequestKind::MemorySearch {
                query: "typed tools".into(),
                limit: 10,
            },
        );
        let outcome = executor
            .execute(ToolExecutionId::new(), &search)
            .await
            .expect("search");
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|value| value.contains("project/decision"))
        );
    }

    #[tokio::test]
    async fn secret_namespace_is_denied_before_repository_write() {
        let repository = InMemoryMemoryRepository::default();
        let executor = MemoryExecutor::new(repository.clone());
        let request = ToolRequest::new(
            mcg_domain::TaskId::new(),
            "secret",
            ToolRequestKind::MemoryWrite {
                key: "secrets/token".into(),
                value: Value::String("fixture".into()),
            },
        );
        assert!(matches!(
            executor.execute(ToolExecutionId::new(), &request).await,
            Err(ToolError::Denied(_))
        ));
        assert!(
            repository
                .read("secrets/token")
                .await
                .expect("read")
                .is_none()
        );
    }
}
