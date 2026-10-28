//! Assembly (spec 0001 §9): one document from a manifest and many chapters.
//!
//! Chapters are found by their `id`, never by file name, so a chapter file can move or be renamed. The order
//! is the manifest's. Each chapter becomes one top-level section, or one appendix.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::finding::Finding;
use crate::model::{
    Appendix, AssetSource, Block, Control, CoverImage, CoverLogo, DataItem, Document, Section,
    TemplateOverrides,
};
use crate::register::{Register, Stage};

/// A document without its body and appendices, plus the chapter ids that make them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub ord: String,
    pub class: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organisation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub control: Control,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<CoverImage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cover_logos: Vec<CoverLogo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_details: Option<Vec<DataItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, AssetSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateOverrides>,
    /// The chapters of the body, in order.
    pub chapters: Vec<String>,
    /// The chapters that become appendices, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub appendix_chapters: Vec<String>,
}

/// One chapter: a top-level section written on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    pub status: ChapterStatus,
    pub body: Vec<Block>,
}

/// How far a chapter's author has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChapterStatus {
    Outline,
    Draft,
    Review,
    Agreed,
}

/// Builds one document. A chapter listed twice, listed and not found, or given twice is `ORD021`; so is a
/// chapter that is not agreed in a document whose status is an issued one (approved, issued, superseded).
/// The result still has to pass the checks like any document.
pub fn assemble(
    manifest: &Manifest,
    chapters: &[Chapter],
    register: &Register,
) -> Result<Document, Vec<Finding>> {
    let mut findings = Vec::new();
    let mut by_id: BTreeMap<&str, &Chapter> = BTreeMap::new();
    for c in chapters {
        if by_id.insert(&c.id, c).is_some() {
            findings.push(Finding::error(
                "ORD021",
                "/chapters",
                format!("two chapters have the id {:?}", c.id),
            ));
        }
    }
    let mut listed = BTreeSet::new();
    let mut take = |list: &[String], field: &str, findings: &mut Vec<Finding>| -> Vec<&Chapter> {
        let mut out = Vec::new();
        for (i, id) in list.iter().enumerate() {
            let path = format!("/{field}/{i}");
            if !listed.insert(id.clone()) {
                findings.push(Finding::error("ORD021", path, format!("the chapter {id:?} is listed twice")));
            } else if let Some(c) = by_id.get(id.as_str()) {
                out.push(*c);
            } else {
                findings.push(Finding::error("ORD021", path, format!("no chapter has the id {id:?}")));
            }
        }
        out
    };
    let body_chapters = take(&manifest.chapters, "chapters", &mut findings);
    let appendix_chapters = take(&manifest.appendix_chapters, "appendix_chapters", &mut findings);

    let issued = register
        .class(&manifest.class)
        .and_then(|c| register.lifecycle(&c.lifecycle))
        .and_then(|l| l.status(&manifest.control.status))
        .is_some_and(|s| s.stage == Stage::Issued);
    if issued {
        for c in body_chapters.iter().chain(&appendix_chapters) {
            if c.status != ChapterStatus::Agreed {
                findings.push(Finding::error(
                    "ORD021",
                    "/control/status",
                    format!(
                        "the document is {:?}, but the chapter {:?} is {:?}, not agreed",
                        manifest.control.status,
                        c.id,
                        format!("{:?}", c.status).to_lowercase()
                    ),
                ));
            }
        }
    }
    if !findings.is_empty() {
        return Err(findings);
    }

    let section = |c: &Chapter| {
        Block::Section(Section {
            id: Some(c.id.clone()),
            title: c.title.clone(),
            numbered: true,
            body: c.body.clone(),
        })
    };
    let m = manifest.clone();
    Ok(Document {
        ord: m.ord,
        class: m.class,
        title: m.title,
        subtitle: m.subtitle,
        organisation: m.organisation,
        language: m.language,
        control: m.control,
        summary: m.summary,
        notice: m.notice,
        contents: m.contents,
        cover_image: m.cover_image,
        cover_logos: m.cover_logos,
        cover_details: m.cover_details,
        footer: m.footer,
        body: body_chapters.iter().map(|c| section(c)).collect(),
        appendices: appendix_chapters
            .iter()
            .map(|c| Appendix { id: Some(c.id.clone()), title: c.title.clone(), body: c.body.clone() })
            .collect(),
        assets: m.assets,
        template: m.template,
    })
}
