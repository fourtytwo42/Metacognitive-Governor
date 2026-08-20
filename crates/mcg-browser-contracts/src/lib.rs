//! Shared, dependency-light browser evidence contracts.
//!
//! The governed consumer browser and the isolated shadow runner use these
//! exact bytes and pass criteria so an automatic benchmark cannot silently
//! weaken or drift from the user-facing accessibility check.

use serde::{Deserialize, Serialize};

pub const ACCESSIBILITY_AUDIT_SCHEMA_VERSION: u16 = 1;

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
}
