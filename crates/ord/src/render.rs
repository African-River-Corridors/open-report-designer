//! Rendering (spec 0001 §11.1 `render`): check, refuse on any error, resolve, then one writer.

use crate::assets::Assets;
use crate::finding::{Finding, has_errors};
use crate::model::Document;
use crate::register::Register;
use crate::resolve::{Resolved, resolve};
use crate::template::Template;

/// An output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Pdf,
    Html,
    Docx,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Pdf, Format::Html, Format::Docx];

    /// Reads "pdf", "html" or "docx" (any case; "word" for docx).
    pub fn parse(s: &str) -> Option<Format> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pdf" => Some(Format::Pdf),
            "html" | "htm" => Some(Format::Html),
            "docx" | "word" => Some(Format::Docx),
            _ => None,
        }
    }

    /// The file extension, without a dot.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Html => "html",
            Format::Docx => "docx",
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Format::Pdf => "application/pdf",
            Format::Html => "text/html; charset=utf-8",
            Format::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        }
    }
}

/// A rendered document and the warnings it raised.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub bytes: Vec<u8>,
    pub warnings: Vec<Finding>,
}

/// What every writer is given: the resolved document, the template and the assets.
#[derive(Debug, Clone)]
pub struct Prepared<'a> {
    pub doc: Resolved,
    pub template: Template,
    pub assets: &'a Assets,
}

/// Checks the document and prepares it for a writer. Any error refuses: the findings come back as `Err`.
/// On success the warnings come back with the prepared document.
pub fn prepare<'a>(
    doc: &Document,
    assets: &'a Assets,
    register: &Register,
) -> Result<(Prepared<'a>, Vec<Finding>), Vec<Finding>> {
    let mut findings = crate::check::check(doc, assets, register);
    let (template, tf) = match &doc.template {
        Some(o) => Template::with_overrides(o, assets),
        None => (Template::default(), Vec::new()),
    };
    findings.extend(tf);
    if has_errors(&findings) {
        return Err(findings);
    }
    let (resolved, _) = resolve(doc, register);
    Ok((Prepared { doc: resolved, template, assets }, findings))
}

/// Renders a document in one format. Any error finding refuses the render.
pub fn render(
    doc: &Document,
    assets: &Assets,
    register: &Register,
    format: Format,
) -> Result<Output, Vec<Finding>> {
    let (p, warnings) = prepare(doc, assets, register)?;
    let bytes = match format {
        Format::Html => crate::write::html::write(&p).into_bytes(),
        Format::Pdf => crate::write::pdf::write(&p)?,
        Format::Docx => crate::write::docx::write(&p)?,
    };
    Ok(Output { bytes, warnings })
}

/// The page furniture that every writer prints the same way.
impl Prepared<'_> {
    /// The left of the running head: "RPT-0042  Rev 1", or "Rev 1" with no number.
    pub fn header_left(&self) -> String {
        let c = &self.doc.control;
        match &c.number {
            Some(n) => format!("{n}  Rev {}", c.revision),
            None => format!("Rev {}", c.revision),
        }
    }

    /// The left of the running foot: the organisation and the notice, joined by " · ".
    pub fn footer_left(&self) -> String {
        [self.doc.organisation.as_deref(), self.doc.notice.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// The document-control block on the cover: number, revision, date and status.
    pub fn cover_control(&self) -> Vec<(&'static str, String)> {
        let c = &self.doc.control;
        let mut v = Vec::new();
        if let Some(n) = &c.number {
            v.push(("Document", n.clone()));
        }
        v.push(("Revision", c.revision.clone()));
        v.push(("Date", c.date.clone()));
        v.push(("Status", c.status.clone()));
        v
    }

    /// A status of the revision history as printed: its label when the lifecycle knows it.
    pub fn status_label(&self, status: &str) -> String {
        self.doc
            .control
            .status_labels
            .get(&crate::register::normalise(status))
            .cloned()
            .unwrap_or_else(|| status.to_string())
    }
}

/// "Page 3 of 12": the right of the running foot. Writers that count pages fill the numbers themselves.
pub const PAGE_OF: (&str, &str) = ("Page ", " of ");
