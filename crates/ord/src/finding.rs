//! Findings: what the checks report about a document.

use serde::Serialize;

/// How bad a finding is. Any error refuses a render; warnings travel with the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One finding: a code from spec 0001 §8, its severity, a JSON pointer into the document and a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

impl Finding {
    pub fn error(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Finding { code, severity: Severity::Error, path: path.into(), message: message.into() }
    }

    pub fn warning(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Finding { code, severity: Severity::Warning, path: path.into(), message: message.into() }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let path = if self.path.is_empty() { "/" } else { &self.path };
        write!(f, "{sev} {} at {path}: {}", self.code, self.message)
    }
}

/// True when any finding is an error.
pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(Finding::is_error)
}
