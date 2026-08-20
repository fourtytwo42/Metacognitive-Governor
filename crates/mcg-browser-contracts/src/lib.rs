//! Shared, dependency-light browser evidence contracts.
//!
//! The governed consumer browser and the isolated shadow runner use these
//! exact bytes and pass criteria so an automatic benchmark cannot silently
//! weaken or drift from the user-facing accessibility check.

use serde::{Deserialize, Serialize};
use url::Url;

pub const ACCESSIBILITY_AUDIT_SCHEMA_VERSION: u16 = 1;
pub const BROWSER_WORKFLOW_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserWorkflowDocument {
    pub schema_version: u16,
    pub name: String,
    pub start_url: String,
    pub steps: Vec<BrowserWorkflowStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserWorkflowStep {
    AssertElement { selector: String },
    Click { selector: String },
    Type { selector: String, value: String },
    Press { selector: String, key: String },
    AssertText { selector: String, contains: String },
    AssertUrl { url: String },
}

impl BrowserWorkflowDocument {
    /// Validates the bounded, same-origin, non-script workflow language.
    ///
    /// # Errors
    ///
    /// Returns a stable explanation for malformed or unsafe workflow input.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != BROWSER_WORKFLOW_SCHEMA_VERSION
            || self.name.trim().is_empty()
            || self.name.chars().count() > 128
            || self.steps.is_empty()
            || self.steps.len() > 64
        {
            return Err(
                "browser workflow requires schema version 1, a bounded name, and 1 to 64 steps"
                    .into(),
            );
        }
        let start = workflow_url(&self.start_url)?;
        let origin = start.origin().ascii_serialization();
        for step in &self.steps {
            match step {
                BrowserWorkflowStep::AssertElement { selector }
                | BrowserWorkflowStep::Click { selector }
                | BrowserWorkflowStep::Type { selector, .. }
                | BrowserWorkflowStep::Press { selector, .. }
                | BrowserWorkflowStep::AssertText { selector, .. } => {
                    validate_workflow_selector(selector)?;
                }
                BrowserWorkflowStep::AssertUrl { url } => {
                    if workflow_url(url)?.origin().ascii_serialization() != origin {
                        return Err(
                            "browser workflow URL assertions must remain on the start origin"
                                .into(),
                        );
                    }
                }
            }
            match step {
                BrowserWorkflowStep::Type { value, .. }
                    if value.chars().count() > 4_096 || value.contains('\0') =>
                {
                    return Err(
                        "browser workflow typed values must be at most 4096 characters".into(),
                    );
                }
                BrowserWorkflowStep::Press { key, .. }
                    if !matches!(
                        key.as_str(),
                        "Enter"
                            | "Tab"
                            | "Escape"
                            | "Space"
                            | "ArrowUp"
                            | "ArrowDown"
                            | "ArrowLeft"
                            | "ArrowRight"
                    ) =>
                {
                    return Err("browser workflow key is not in the safe key allowlist".into());
                }
                BrowserWorkflowStep::AssertText { contains, .. }
                    if contains.trim().is_empty() || contains.chars().count() > 1_000 =>
                {
                    return Err(
                        "browser workflow text assertions must contain 1 to 1000 characters".into(),
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn workflow_url(value: &str) -> Result<Url, String> {
    let url =
        Url::parse(value).map_err(|error| format!("invalid browser workflow URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("browser workflow URLs must use credential-free HTTP(S)".into());
    }
    Ok(url)
}

fn validate_workflow_selector(selector: &str) -> Result<(), String> {
    if selector.trim().is_empty()
        || selector.chars().count() > 512
        || selector.chars().any(char::is_control)
    {
        return Err("browser selector must contain 1 to 512 printable characters".into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityAudit {
    pub schema_version: u16,
    pub document_language_present: bool,
    pub non_empty_title: bool,
    pub main_landmarks: u32,
    pub images_missing_alt: u32,
    pub controls_without_name: u32,
    pub interactive_without_name: u32,
    pub duplicate_ids: u32,
    pub heading_order_violations: u32,
    pub positive_tabindex: u32,
}

impl AccessibilityAudit {
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.schema_version == ACCESSIBILITY_AUDIT_SCHEMA_VERSION
            && self.document_language_present
            && self.non_empty_title
            && self.main_landmarks == 1
            && self.images_missing_alt == 0
            && self.controls_without_name == 0
            && self.interactive_without_name == 0
            && self.duplicate_ids == 0
            && self.heading_order_violations == 0
            && self.positive_tabindex == 0
    }

    #[must_use]
    pub fn detail(&self) -> String {
        format!(
            "language={}; title={}; main_landmarks={}; images_missing_alt={}; controls_without_name={}; interactive_without_name={}; duplicate_ids={}; heading_order_violations={}; positive_tabindex={}",
            self.document_language_present,
            self.non_empty_title,
            self.main_landmarks,
            self.images_missing_alt,
            self.controls_without_name,
            self.interactive_without_name,
            self.duplicate_ids,
            self.heading_order_violations,
            self.positive_tabindex
        )
    }
}

pub const ACCESSIBILITY_AUDIT_SCRIPT: &str = r#"
(() => {
  const text = (element) => (element?.textContent || '').trim();
  const labelledBy = (element) => (element.getAttribute('aria-labelledby') || '')
    .split(/\s+/)
    .filter(Boolean)
    .map((id) => text(document.getElementById(id)))
    .join(' ')
    .trim();
  const hasName = (element) => Boolean(
    (element.getAttribute('aria-label') || '').trim()
    || labelledBy(element)
    || text(element)
    || (element.getAttribute('alt') || '').trim()
    || (element.getAttribute('title') || '').trim()
    || (element.getAttribute('value') || '').trim()
  );
  const controlsWithoutName = Array.from(
    document.querySelectorAll("input:not([type='hidden']), select, textarea")
  ).filter((element) => {
    const labels = Array.from(element.labels || []).some((label) => text(label));
    return !labels && !hasName(element);
  }).length;
  const interactiveWithoutName = Array.from(
    document.querySelectorAll("button, a[href], [role='button'], [role='link']")
  ).filter((element) => !hasName(element)).length;
  const ids = new Set();
  let duplicateIds = 0;
  for (const element of document.querySelectorAll('[id]')) {
    if (!element.id) continue;
    if (ids.has(element.id)) duplicateIds += 1;
    ids.add(element.id);
  }
  let previousHeading = 0;
  let headingOrderViolations = 0;
  for (const heading of document.querySelectorAll('h1,h2,h3,h4,h5,h6')) {
    const level = Number(heading.tagName.slice(1));
    if (previousHeading && level > previousHeading + 1) headingOrderViolations += 1;
    previousHeading = level;
  }
  return {
    schemaVersion: 1,
    documentLanguagePresent: Boolean((document.documentElement.lang || '').trim()),
    nonEmptyTitle: Boolean((document.title || '').trim()),
    mainLandmarks: document.querySelectorAll('main, [role="main"]').length,
    imagesMissingAlt: document.querySelectorAll('img:not([alt])').length,
    controlsWithoutName,
    interactiveWithoutName,
    duplicateIds,
    headingOrderViolations,
    positiveTabindex: document.querySelectorAll('[tabindex]:not([tabindex="0"]):not([tabindex="-1"])').length
  };
})()
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_contract_requires_every_objective_invariant() {
        let passing = AccessibilityAudit {
            schema_version: ACCESSIBILITY_AUDIT_SCHEMA_VERSION,
            document_language_present: true,
            non_empty_title: true,
            main_landmarks: 1,
            images_missing_alt: 0,
            controls_without_name: 0,
            interactive_without_name: 0,
            duplicate_ids: 0,
            heading_order_violations: 0,
            positive_tabindex: 0,
        };
        assert!(passing.passed());
        let mut failing = passing.clone();
        failing.controls_without_name = 1;
        assert!(!failing.passed());
        failing = passing.clone();
        failing.schema_version += 1;
        assert!(!failing.passed());
        assert!(passing.detail().contains("main_landmarks=1"));
    }

    #[test]
    fn accessibility_contract_round_trips_the_browser_shape() {
        let value = serde_json::json!({
            "schemaVersion": 1,
            "documentLanguagePresent": true,
            "nonEmptyTitle": true,
            "mainLandmarks": 1,
            "imagesMissingAlt": 0,
            "controlsWithoutName": 0,
            "interactiveWithoutName": 0,
            "duplicateIds": 0,
            "headingOrderViolations": 0,
            "positiveTabindex": 0
        });
        let audit: AccessibilityAudit = serde_json::from_value(value).expect("audit");
        assert!(audit.passed());
    }

    #[test]
    fn workflow_contract_rejects_cross_origin_scriptable_or_unbounded_steps() {
        let mut workflow = BrowserWorkflowDocument {
            schema_version: BROWSER_WORKFLOW_SCHEMA_VERSION,
            name: "consumer flow".into(),
            start_url: "http://127.0.0.1:43123/app".into(),
            steps: vec![
                BrowserWorkflowStep::Type {
                    selector: "#name".into(),
                    value: "Ada".into(),
                },
                BrowserWorkflowStep::Press {
                    selector: "#name".into(),
                    key: "Enter".into(),
                },
                BrowserWorkflowStep::AssertUrl {
                    url: "http://127.0.0.1:43123/done".into(),
                },
            ],
        };
        assert!(workflow.validate().is_ok());
        workflow.steps[1] = BrowserWorkflowStep::Press {
            selector: "#name".into(),
            key: "Control+L".into(),
        };
        assert!(workflow.validate().is_err());
        workflow.steps[1] = BrowserWorkflowStep::AssertUrl {
            url: "https://example.com/escape".into(),
        };
        assert!(workflow.validate().is_err());
    }
}
