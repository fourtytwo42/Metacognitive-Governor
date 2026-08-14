use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use mcg_domain::ToolExecutionId;
use reqwest::Url;
use sha2::{Digest, Sha256};

use crate::{
    HttpExecutor, RESEARCH_EVIDENCE_SCHEMA_VERSION, ResearchEvidenceDocument, TOOL_SCHEMA_VERSION,
    ToolError, ToolExecutor, ToolOutcome, ToolOutcomeStatus, ToolRequest, ToolRequestKind,
    VerificationEvidence,
};

const MAX_SOURCES: usize = 32;
const MAX_CLAIMS: usize = 128;
const MAX_CITATIONS: usize = 256;
const MAX_QUOTE_CHARS: usize = 1_000;
const MAX_TEXT_CHARS: usize = 8_000;

pub struct ResearchExecutor {
    http: Arc<HttpExecutor>,
}

impl ResearchExecutor {
    #[must_use]
    pub fn new(http: Arc<HttpExecutor>) -> Self {
        Self { http }
    }

    #[allow(clippy::too_many_lines)] // Fetching and evaluating one immutable evidence bundle stays atomic.
    async fn verify(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
        parameters: ResearchParameters<'_>,
    ) -> Result<ToolOutcome, ToolError> {
        let started_at = Utc::now();
        let validation = validate_document(&parameters);
        let source_urls = match validation {
            Ok(urls) => urls,
            Err(detail) => {
                return Ok(failed_outcome(
                    execution_id,
                    request,
                    started_at,
                    parameters.evidence_artifact.clone(),
                    detail,
                    Vec::new(),
                ));
            }
        };

        let observed_evidence_sha = format!("{:x}", Sha256::digest(parameters.evidence_json));
        if !observed_evidence_sha.eq_ignore_ascii_case(&parameters.evidence_artifact.sha256) {
            return Ok(failed_outcome(
                execution_id,
                request,
                started_at,
                parameters.evidence_artifact.clone(),
                "research evidence JSON does not match its immutable artifact".into(),
                Vec::new(),
            ));
        }
        if !matches!(
            serde_json::from_str::<ResearchEvidenceDocument>(parameters.evidence_json),
            Ok(document) if document == *parameters.document
        ) {
            return Ok(failed_outcome(
                execution_id,
                request,
                started_at,
                parameters.evidence_artifact.clone(),
                "parsed research evidence does not match the immutable JSON bytes".into(),
                Vec::new(),
            ));
        }

        let mut bodies = BTreeMap::new();
        let mut artifacts = vec![parameters.evidence_artifact.clone()];
        let mut source_verification = Vec::new();
        let deadline =
            tokio::time::Instant::now() + std::time::Duration::from_secs(request.timeout_seconds);
        for (index, url) in source_urls.iter().enumerate() {
            let mut source_request = ToolRequest::new(
                request.task_id,
                format!("fetch research source {}", index + 1),
                ToolRequestKind::HttpRequest {
                    method: "GET".into(),
                    url: url.clone(),
                    headers: Vec::new(),
                    body: None,
                    max_response_bytes: parameters.max_response_bytes,
                },
            );
            source_request.timeout_seconds = request.timeout_seconds;
            let outcome =
                tokio::time::timeout_at(deadline, self.http.execute(execution_id, &source_request))
                    .await
                    .map_err(|_| ToolError::TimedOut)??;
            if let Some(artifact) = outcome.artifacts.first() {
                source_verification.push(VerificationEvidence {
                    method: "research.source".into(),
                    passed: outcome.status == ToolOutcomeStatus::Succeeded,
                    detail: format!("{} sha256={}", sanitized_source_url(url), artifact.sha256),
                });
            }
            artifacts.extend(outcome.artifacts);
            if outcome.status != ToolOutcomeStatus::Succeeded {
                let mut failed = failed_outcome(
                    execution_id,
                    request,
                    started_at,
                    parameters.evidence_artifact.clone(),
                    format!("research source returned a non-success response: {url}"),
                    artifacts.into_iter().skip(1).collect(),
                );
                failed.verification.extend(source_verification);
                return Ok(failed);
            }
            bodies.insert(url.clone(), outcome.stdout.unwrap_or_default());
        }

        for claim in &parameters.document.claims {
            for citation in &claim.citations {
                let body = bodies.get(&citation.url).ok_or_else(|| {
                    ToolError::Execution("validated research source was not fetched".into())
                })?;
                if !quote_is_grounded(body, &citation.quote) {
                    let mut failed = failed_outcome(
                        execution_id,
                        request,
                        started_at,
                        parameters.evidence_artifact.clone(),
                        format!(
                            "citation quote for claim {} was not found in its fetched source",
                            claim.id
                        ),
                        artifacts.into_iter().skip(1).collect(),
                    );
                    failed.verification.extend(source_verification);
                    return Ok(failed);
                }
            }
        }

        let citation_count = parameters
            .document
            .claims
            .iter()
            .map(|claim| claim.citations.len())
            .sum::<usize>();
        let freshness = parameters.max_age_days.map_or_else(
            || "not required".into(),
            |days| format!("all citations published within {days} days"),
        );
        let mut verification = vec![
            VerificationEvidence {
                method: "research.scope".into(),
                passed: true,
                detail: "evidence scope exactly matches the task goal".into(),
            },
            VerificationEvidence {
                method: "research.sources".into(),
                passed: true,
                detail: format!("{} distinct sources fetched", source_urls.len()),
            },
            VerificationEvidence {
                method: "research.citations".into(),
                passed: true,
                detail: format!("{citation_count} citation quotes grounded"),
            },
            VerificationEvidence {
                method: "research.freshness".into(),
                passed: true,
                detail: freshness,
            },
            VerificationEvidence {
                method: "research.conflicts".into(),
                passed: true,
                detail: format!(
                    "{} declared conflict records validated",
                    parameters.document.conflicts.len()
                ),
            },
        ];
        verification.extend(source_verification);
        Ok(ToolOutcome {
            schema_version: TOOL_SCHEMA_VERSION,
            execution_id,
            request_id: request.id,
            status: ToolOutcomeStatus::Succeeded,
            summary: format!(
                "verified {} research claims against {} fetched sources",
                parameters.document.claims.len(),
                source_urls.len()
            ),
            exit_code: None,
            stdout: None,
            stderr: None,
            artifacts,
            verification,
            started_at,
            finished_at: Utc::now(),
        })
    }
}

#[async_trait]
impl ToolExecutor for ResearchExecutor {
    async fn execute(
        &self,
        execution_id: ToolExecutionId,
        request: &ToolRequest,
    ) -> Result<ToolOutcome, ToolError> {
        match &request.kind {
            ToolRequestKind::ResearchVerify {
                evidence_path,
                evidence_artifact,
                evidence_json,
                document,
                required_scope,
                minimum_sources,
                max_age_days,
                max_response_bytes,
            } => {
                self.verify(
                    execution_id,
                    request,
                    ResearchParameters {
                        evidence_path,
                        evidence_artifact,
                        evidence_json,
                        document,
                        required_scope,
                        minimum_sources: *minimum_sources,
                        max_age_days: *max_age_days,
                        max_response_bytes: *max_response_bytes,
                    },
                )
                .await
            }
            _ => Err(ToolError::Invalid(
                "research executor received an unsupported tool".into(),
            )),
        }
    }
}

struct ResearchParameters<'a> {
    evidence_path: &'a str,
    evidence_artifact: &'a crate::ArtifactReference,
    evidence_json: &'a str,
    document: &'a ResearchEvidenceDocument,
    required_scope: &'a str,
    minimum_sources: u16,
    max_age_days: Option<u32>,
    max_response_bytes: u64,
}

#[allow(clippy::too_many_lines)] // All cross-field research invariants are checked in one pass.
fn validate_document(parameters: &ResearchParameters<'_>) -> Result<Vec<String>, String> {
    if parameters.evidence_path.trim().is_empty() {
        return Err("research evidence path is required".into());
    }
    if parameters.document.schema_version != RESEARCH_EVIDENCE_SCHEMA_VERSION {
        return Err("unsupported research evidence schema version".into());
    }
    if parameters.required_scope.trim().is_empty()
        || parameters.document.scope.trim() != parameters.required_scope.trim()
    {
        return Err("research evidence scope does not exactly match the task goal".into());
    }
    if parameters.minimum_sources == 0 || usize::from(parameters.minimum_sources) > MAX_SOURCES {
        return Err(format!(
            "minimum_sources must be between 1 and {MAX_SOURCES}"
        ));
    }
    if parameters.max_response_bytes == 0 || parameters.max_response_bytes > 1_048_576 {
        return Err("research source byte limit must be between 1 and 1048576".into());
    }
    if parameters.document.claims.is_empty() || parameters.document.claims.len() > MAX_CLAIMS {
        return Err(format!(
            "research evidence must contain between 1 and {MAX_CLAIMS} claims"
        ));
    }

    let now = Utc::now();
    let cutoff = if let Some(days) = parameters.max_age_days {
        if days == 0 || days > 36_500 {
            return Err("max_age_days must be between 1 and 36500".into());
        }
        Some(now - Duration::days(i64::from(days)))
    } else {
        None
    };
    let future_limit = now + Duration::days(1);
    let mut claim_ids = BTreeSet::new();
    let mut urls = BTreeSet::new();
    let mut citation_count = 0_usize;
    for claim in &parameters.document.claims {
        if claim.id.trim().is_empty()
            || claim.id.chars().count() > 128
            || !claim_ids.insert(claim.id.clone())
        {
            return Err("claim ids must be non-empty, unique, and at most 128 characters".into());
        }
        if claim.statement.trim().is_empty() || claim.statement.chars().count() > MAX_TEXT_CHARS {
            return Err("claim statements must be non-empty and bounded".into());
        }
        if claim.citations.is_empty() {
            return Err(format!("claim {} has no citations", claim.id));
        }
        citation_count = citation_count.saturating_add(claim.citations.len());
        if citation_count > MAX_CITATIONS {
            return Err(format!(
                "research evidence exceeds {MAX_CITATIONS} citations"
            ));
        }
        for citation in &claim.citations {
            validate_source_url(&citation.url)?;
            let quote_chars = citation.quote.trim().chars().count();
            if !(8..=MAX_QUOTE_CHARS).contains(&quote_chars) {
                return Err(format!(
                    "citation quotes must be between 8 and {MAX_QUOTE_CHARS} characters"
                ));
            }
            if citation
                .published_at
                .is_some_and(|published| published > future_limit)
            {
                return Err(format!(
                    "citation for claim {} has an implausible future date",
                    claim.id
                ));
            }
            if let Some(cutoff) = cutoff {
                let published = citation.published_at.ok_or_else(|| {
                    "published_at is required when max_age_days is configured".to_string()
                })?;
                if published < cutoff {
                    return Err(format!("citation for claim {} is too old", claim.id));
                }
            }
            urls.insert(citation.url.clone());
        }
    }
    if urls.len() < usize::from(parameters.minimum_sources) || urls.len() > MAX_SOURCES {
        return Err(format!(
            "research evidence has {} distinct sources; required {} to {MAX_SOURCES}",
            urls.len(),
            parameters.minimum_sources
        ));
    }

    for conflict in &parameters.document.conflicts {
        if conflict.summary.trim().is_empty() || conflict.summary.chars().count() > MAX_TEXT_CHARS {
            return Err("conflict summaries must be non-empty and bounded".into());
        }
        let referenced = conflict.claim_ids.iter().collect::<BTreeSet<_>>();
        if referenced.len() < 2
            || conflict
                .claim_ids
                .iter()
                .any(|claim_id| !claim_ids.contains(claim_id))
        {
            return Err("each conflict must reference at least two distinct known claims".into());
        }
    }
    Ok(urls.into_iter().collect())
}

fn validate_source_url(value: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|error| format!("invalid research source URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("research sources must use credential-free HTTP(S) URLs".into());
    }
    Ok(())
}

fn quote_is_grounded(body: &str, quote: &str) -> bool {
    let quote = normalize_text(quote);
    normalize_text(body).contains(&quote)
        || normalize_text(&decode_common_html_entities(&strip_html_tags(body))).contains(&quote)
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn strip_html_tags(value: &str) -> String {
    let mut text = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text
}

fn decode_common_html_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn sanitized_source_url(value: &str) -> String {
    Url::parse(value).map_or_else(
        |_| "invalid-url".into(),
        |mut url| {
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        },
    )
}

fn failed_outcome(
    execution_id: ToolExecutionId,
    request: &ToolRequest,
    started_at: chrono::DateTime<Utc>,
    evidence_artifact: crate::ArtifactReference,
    detail: String,
    source_artifacts: Vec<crate::ArtifactReference>,
) -> ToolOutcome {
    let mut artifacts = vec![evidence_artifact];
    artifacts.extend(source_artifacts);
    ToolOutcome {
        schema_version: TOOL_SCHEMA_VERSION,
        execution_id,
        request_id: request.id,
        status: ToolOutcomeStatus::Failed,
        summary: detail.clone(),
        exit_code: None,
        stdout: None,
        stderr: None,
        artifacts,
        verification: vec![VerificationEvidence {
            method: "research.verify".into(),
            passed: false,
            detail,
        }],
        started_at,
        finished_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::routing::get;
    use tempfile::tempdir;

    use super::*;
    use crate::{ArtifactStore, FilesystemExecutor, RejectingCredentialResolver};

    async fn server() -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new()
            .route(
                "/one",
                get(|| async { "<p>The governed result is reproducible.</p>" }),
            )
            .route(
                "/two",
                get(|| async { "Independent evidence confirms the result." }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let task = tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
        (format!("http://{address}"), task)
    }

    fn executor(project: &std::path::Path, artifacts: &std::path::Path) -> ResearchExecutor {
        let filesystem = Arc::new(
            FilesystemExecutor::open([project.to_path_buf()], artifacts).expect("filesystem"),
        );
        let http = Arc::new(
            HttpExecutor::new(Arc::new(RejectingCredentialResolver), filesystem, artifacts)
                .expect("http"),
        );
        ResearchExecutor::new(http)
    }

    fn request(base: &str, artifacts: &ArtifactStore, second_quote: &str) -> ToolRequest {
        let document = ResearchEvidenceDocument {
            schema_version: RESEARCH_EVIDENCE_SCHEMA_VERSION,
            scope: "answer the question".into(),
            claims: vec![
                crate::ResearchClaimEvidence {
                    id: "one".into(),
                    statement: "first claim".into(),
                    citations: vec![crate::ResearchCitationEvidence {
                        url: format!("{base}/one"),
                        quote: "The governed result is reproducible.".into(),
                        published_at: Some(Utc::now()),
                    }],
                },
                crate::ResearchClaimEvidence {
                    id: "two".into(),
                    statement: "second claim".into(),
                    citations: vec![crate::ResearchCitationEvidence {
                        url: format!("{base}/two"),
                        quote: second_quote.into(),
                        published_at: Some(Utc::now()),
                    }],
                },
            ],
            conflicts: vec![crate::ResearchConflictEvidence {
                claim_ids: vec!["one".into(), "two".into()],
                summary: "the sources use different methods".into(),
            }],
        };
        let evidence_json = serde_json::to_string(&document).expect("serialize evidence");
        let artifact = artifacts
            .capture(evidence_json.as_bytes(), "application/json")
            .expect("artifact");
        ToolRequest::new(
            mcg_domain::TaskId::new(),
            "verify research",
            ToolRequestKind::ResearchVerify {
                evidence_path: "research.json".into(),
                evidence_artifact: artifact,
                evidence_json,
                document,
                required_scope: "answer the question".into(),
                minimum_sources: 2,
                max_age_days: Some(30),
                max_response_bytes: 1_048_576,
            },
        )
    }

    #[tokio::test]
    async fn grounded_sources_pass_and_retain_every_artifact() {
        let (base, server) = server().await;
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let store = ArtifactStore::open(artifacts.path()).expect("store");
        let request = request(&base, &store, "Independent evidence confirms the result.");
        let expected_sha = match &request.kind {
            ToolRequestKind::ResearchVerify {
                evidence_artifact, ..
            } => evidence_artifact.sha256.clone(),
            _ => unreachable!(),
        };
        let outcome = executor(project.path(), artifacts.path())
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("verify");
        assert_eq!(outcome.status, ToolOutcomeStatus::Succeeded);
        assert_eq!(outcome.artifacts.len(), 3);
        assert_eq!(outcome.artifacts[0].sha256, expected_sha);
        assert_eq!(
            outcome
                .verification
                .iter()
                .filter(|evidence| evidence.method == "research.source")
                .count(),
            2
        );
        server.abort();
    }

    #[tokio::test]
    async fn ungrounded_quote_fails_with_fetched_evidence() {
        let (base, server) = server().await;
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let store = ArtifactStore::open(artifacts.path()).expect("store");
        let request = request(&base, &store, "This quote is absent from the source.");
        let outcome = executor(project.path(), artifacts.path())
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("verify");
        assert_eq!(outcome.status, ToolOutcomeStatus::Failed);
        assert_eq!(outcome.artifacts.len(), 3);
        assert!(outcome.summary.contains("not found"));
        server.abort();
    }

    #[tokio::test]
    async fn evidence_bytes_cannot_change_after_artifact_binding() {
        let project = tempdir().expect("project");
        let artifacts = tempdir().expect("artifacts");
        let store = ArtifactStore::open(artifacts.path()).expect("store");
        let mut request = request(
            "https://example.invalid",
            &store,
            "an independent grounded quotation",
        );
        let ToolRequestKind::ResearchVerify { evidence_json, .. } = &mut request.kind else {
            panic!("research request")
        };
        evidence_json.push(' ');
        let outcome = executor(project.path(), artifacts.path())
            .execute(ToolExecutionId::new(), &request)
            .await
            .expect("verify");
        assert_eq!(outcome.status, ToolOutcomeStatus::Failed);
        assert_eq!(outcome.artifacts.len(), 1);
        assert!(outcome.summary.contains("immutable artifact"));
    }

    #[test]
    fn malformed_conflicts_and_stale_citations_are_rejected() {
        let artifacts = tempdir().expect("artifacts");
        let store = ArtifactStore::open(artifacts.path()).expect("store");
        let mut request = request("https://example.com", &store, "a grounded second quote");
        let ToolRequestKind::ResearchVerify {
            evidence_artifact,
            evidence_json,
            document,
            max_age_days,
            ..
        } = &mut request.kind
        else {
            panic!("research request")
        };
        document.conflicts[0].claim_ids = vec!["one".into(), "missing".into()];
        {
            let parameters = ResearchParameters {
                evidence_path: "research.json",
                evidence_artifact,
                evidence_json,
                document,
                required_scope: "answer the question",
                minimum_sources: 2,
                max_age_days: *max_age_days,
                max_response_bytes: 1_048_576,
            };
            assert!(
                validate_document(&parameters)
                    .expect_err("invalid conflict")
                    .contains("known claims")
            );
        }

        document.conflicts[0].claim_ids = vec!["one".into(), "two".into()];
        document.claims[0].citations[0].published_at = Some(Utc::now() - Duration::days(31));
        let parameters = ResearchParameters {
            evidence_path: "research.json",
            evidence_artifact,
            evidence_json,
            document,
            required_scope: "answer the question",
            minimum_sources: 2,
            max_age_days: *max_age_days,
            max_response_bytes: 1_048_576,
        };
        assert!(
            validate_document(&parameters)
                .expect_err("stale citation")
                .contains("too old")
        );
    }

    #[test]
    fn quote_grounding_uses_visible_html_text_and_common_entities() {
        assert!(quote_is_grounded(
            "<p>Governed <strong>research</strong> &amp; evidence</p>",
            "Governed research & evidence"
        ));
    }
}
