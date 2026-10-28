//! The register of document classes and their lifecycles (spec 0001 §4).
//!
//! Rules are data: a class names its lifecycle, and a lifecycle lists its statuses with the revision form and
//! the sign-offs each one needs. The built-in register serves most reports; a tool may pass its own, which
//! replaces it.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::finding::Finding;

/// A register: document classes and the lifecycles they use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Register {
    pub classes: Vec<DocClass>,
    pub lifecycles: Vec<Lifecycle>,
}

/// A document class, such as "report".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocClass {
    /// Lower-case id.
    pub id: String,
    /// Display name, printed on the cover.
    pub name: String,
    /// A document number must start with "<prefix>-".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_prefix: Option<String>,
    /// The id of a lifecycle in the same register.
    pub lifecycle: String,
    /// Default for the table of contents.
    #[serde(default)]
    pub contents: bool,
    /// Other names that resolve to this class.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

/// A lifecycle: the statuses a document may take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Lifecycle {
    pub id: String,
    /// In order, from the least to the most advanced.
    pub statuses: Vec<Status>,
    /// Other spellings, normalised (lower case, hyphens), to a status id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
}

/// One status of a lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Lower-case id, such as "for-approval".
    pub id: String,
    /// Display label, such as "For approval".
    pub label: String,
    pub stage: Stage,
    pub revision: RevisionForm,
    /// The sign-offs this status needs: all of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<SignOff>,
    /// Text printed on every page while the document has this status. None: nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furniture: Option<String>,
}

/// Whether a status is still being worked on or has been issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Working,
    Issued,
}

/// The form of a revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RevisionForm {
    /// `A` to `Z`, then `AA` to `ZZ`.
    Letter,
    /// A whole number without leading zeros: `0`, `1`, `2`, ...
    Number,
    /// (0.2) Any revision with no white space at its ends and none inside, such as `0.2` or `P01`.
    Text,
}

impl RevisionForm {
    /// True when `revision` has this form.
    pub fn fits(self, revision: &str) -> bool {
        let b = revision.as_bytes();
        match self {
            RevisionForm::Letter => (1..=2).contains(&b.len()) && b.iter().all(u8::is_ascii_uppercase),
            RevisionForm::Number => {
                !b.is_empty() && b.iter().all(u8::is_ascii_digit) && (b.len() == 1 || b[0] != b'0')
            }
            RevisionForm::Text => !revision.is_empty() && !revision.chars().any(char::is_whitespace),
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            RevisionForm::Letter => "a letter revision (A to Z, then AA to ZZ)",
            RevisionForm::Number => "a number revision (0, 1, 2, ... without leading zeros)",
            RevisionForm::Text => "a revision with no spaces (such as 0.2)",
        }
    }
}

/// A document-control field that a status can require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SignOff {
    Prepared,
    Checked,
    Approved,
}

impl SignOff {
    pub fn field(self) -> &'static str {
        match self {
            SignOff::Prepared => "prepared",
            SignOff::Checked => "checked",
            SignOff::Approved => "approved",
        }
    }
}

/// Lower case, trimmed, with spaces and underscores as hyphens.
pub fn normalise(name: &str) -> String {
    name.trim().to_lowercase().split([' ', '_', '-']).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-")
}

impl Register {
    /// Finds a class by id, then by alias. Case, spaces and underscores do not matter.
    pub fn class(&self, name: &str) -> Option<&DocClass> {
        let n = normalise(name);
        self.classes
            .iter()
            .find(|c| c.id == n)
            .or_else(|| self.classes.iter().find(|c| c.aliases.iter().any(|a| normalise(a) == n)))
    }

    pub fn lifecycle(&self, id: &str) -> Option<&Lifecycle> {
        self.lifecycles.iter().find(|l| l.id == id)
    }

    /// The class ids, for messages.
    pub fn class_ids(&self) -> Vec<&str> {
        self.classes.iter().map(|c| c.id.as_str()).collect()
    }

    /// Reads a register from JSON and checks that it holds together: unique ids, every class's lifecycle
    /// exists, every alias names a status. Problems are `ORD002` findings whose path starts `register:`.
    pub fn from_json(json: &str) -> Result<Register, Vec<Finding>> {
        let reg: Register = crate::parse::parse_as(json).map_err(|fs| {
            fs.into_iter()
                .map(|f| Finding::error("ORD002", format!("register:{}", f.path), f.message))
                .collect::<Vec<_>>()
        })?;
        let problems = reg.problems();
        if problems.is_empty() { Ok(reg) } else { Err(problems) }
    }

    fn problems(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        let mut err =
            |path: String, msg: String| out.push(Finding::error("ORD002", format!("register:{path}"), msg));
        let mut seen = BTreeSet::new();
        for (i, c) in self.classes.iter().enumerate() {
            if normalise(&c.id) != c.id {
                err(
                    format!("/classes/{i}/id"),
                    format!("class id {:?} must be lower case with hyphens", c.id),
                );
            }
            if !seen.insert(c.id.clone()) {
                err(format!("/classes/{i}/id"), format!("class id {:?} is listed twice", c.id));
            }
            if self.lifecycle(&c.lifecycle).is_none() {
                err(
                    format!("/classes/{i}/lifecycle"),
                    format!("class {:?} names no lifecycle {:?}", c.id, c.lifecycle),
                );
            }
        }
        for (i, l) in self.lifecycles.iter().enumerate() {
            if l.statuses.is_empty() {
                err(format!("/lifecycles/{i}/statuses"), format!("lifecycle {:?} has no status", l.id));
            }
            let mut ids = BTreeSet::new();
            for (j, s) in l.statuses.iter().enumerate() {
                if normalise(&s.id) != s.id || !ids.insert(s.id.as_str()) {
                    err(
                        format!("/lifecycles/{i}/statuses/{j}/id"),
                        format!("status id {:?} is not lower case with hyphens, or is listed twice", s.id),
                    );
                }
            }
            for (alias, target) in &l.aliases {
                if !ids.contains(target.as_str()) {
                    err(
                        format!("/lifecycles/{i}/aliases"),
                        format!("alias {alias:?} names no status {target:?}"),
                    );
                }
            }
        }
        out
    }
}

impl Lifecycle {
    /// Finds a status by id or alias, after `normalise`.
    pub fn status(&self, name: &str) -> Option<&Status> {
        let n = normalise(name);
        let id = self.aliases.iter().find(|(a, _)| normalise(a) == n).map(|(_, t)| t.as_str()).unwrap_or(&n);
        self.statuses.iter().find(|s| s.id == id)
    }

    pub fn status_ids(&self) -> Vec<&str> {
        self.statuses.iter().map(|s| s.id.as_str()).collect()
    }
}

/// The built-in register (spec 0001 §4): four classes and the `standard` lifecycle.
pub fn builtin_register() -> Register {
    fn class(id: &str, name: &str, prefix: &str, contents: bool) -> DocClass {
        DocClass {
            id: id.into(),
            name: name.into(),
            number_prefix: Some(prefix.into()),
            lifecycle: "standard".into(),
            contents,
            aliases: Vec::new(),
        }
    }
    fn status(id: &str, label: &str, stage: Stage, needs: &[SignOff], furniture: Option<&str>) -> Status {
        let revision = if stage == Stage::Working { RevisionForm::Letter } else { RevisionForm::Number };
        Status {
            id: id.into(),
            label: label.into(),
            stage,
            revision,
            needs: needs.to_vec(),
            furniture: furniture.map(Into::into),
        }
    }
    use SignOff::*;
    use Stage::*;
    let all = [Prepared, Checked, Approved];
    let mut technical_note = class("technical-note", "Technical note", "TN", false);
    technical_note.aliases = vec!["note".into(), "tech note".into()];
    let mut run_report = class("run-report", "Run report", "RUN", false);
    run_report.aliases = vec!["run".into()];
    Register {
        classes: vec![
            class("report", "Report", "RPT", true),
            technical_note,
            class("schedule", "Schedule", "SCH", false),
            run_report,
        ],
        lifecycles: vec![Lifecycle {
            id: "standard".into(),
            statuses: vec![
                status("draft", "Draft", Working, &[], Some("Draft")),
                status("for-checking", "For checking", Working, &[Prepared], Some("For checking")),
                status("for-approval", "For approval", Working, &[Prepared, Checked], Some("For approval")),
                status("approved", "Approved", Issued, &all, None),
                status("issued", "Issued", Issued, &all, None),
                status("superseded", "Superseded", Issued, &all, Some("Superseded")),
            ],
            aliases: [("for-sign-off", "for-approval"), ("for-review", "for-checking"), ("final", "issued")]
                .into_iter()
                .map(|(a, t)| (a.to_string(), t.to_string()))
                .collect(),
        }],
    }
}
