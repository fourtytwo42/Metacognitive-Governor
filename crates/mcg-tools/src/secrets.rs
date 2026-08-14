use std::fmt;

use async_trait::async_trait;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{CredentialReference, ToolError};

#[async_trait]
pub trait CredentialResolver: Send + Sync {
    async fn resolve(&self, reference: &CredentialReference) -> Result<SecretMaterial, ToolError>;
}

#[derive(Debug, Default)]
pub struct RejectingCredentialResolver;

#[async_trait]
impl CredentialResolver for RejectingCredentialResolver {
    async fn resolve(&self, reference: &CredentialReference) -> Result<SecretMaterial, ToolError> {
        Err(ToolError::Denied(format!(
            "credential reference {}/{} is unavailable",
            reference.provider, reference.key
        )))
    }
}

/// Executor-only secret material. It cannot be serialized and debug output is redacted.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretMaterial(String);

impl SecretMaterial {
    /// Creates non-empty secret material.
    ///
    /// # Errors
    ///
    /// Returns an error if an empty value is supplied.
    pub fn new(value: impl Into<String>) -> Result<Self, ToolError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ToolError::Invalid("secret material cannot be empty".into()));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn expose_to_executor(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretMaterial([REDACTED])")
    }
}

#[derive(Debug, Clone, Default)]
pub struct OutputRedactor {
    literals: Vec<SecretMaterial>,
}

impl OutputRedactor {
    #[must_use]
    pub fn new(literals: Vec<SecretMaterial>) -> Self {
        Self { literals }
    }

    #[must_use]
    pub fn redact(&self, value: &str) -> String {
        self.literals
            .iter()
            .fold(value.to_owned(), |output, secret| {
                output.replace(secret.expose_to_executor(), "[REDACTED]")
            })
    }

    pub fn redact_outcome(&self, outcome: &mut crate::ToolOutcome) {
        outcome.summary = self.redact(&outcome.summary);
        outcome.stdout = outcome.stdout.as_deref().map(|value| self.redact(value));
        outcome.stderr = outcome.stderr.as_deref().map(|value| self.redact(value));
        for evidence in &mut outcome.verification {
            evidence.detail = self.redact(&evidence.detail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_and_output_are_redacted() {
        let secret = SecretMaterial::new("sensitive-fixture-value").expect("secret");
        assert!(!format!("{secret:?}").contains(secret.expose_to_executor()));
        let redactor = OutputRedactor::new(vec![secret]);
        assert_eq!(
            redactor.redact("token=sensitive-fixture-value"),
            "token=[REDACTED]"
        );
    }
}
