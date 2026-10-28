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
    /// The left of the running head: "RPT-0042  Rev 1", or "Rev 1" with no number. The word "Rev " is the
    /// template's `revision_prefix`.
    pub fn header_left(&self) -> String {
        let c = &self.doc.control;
        let rev = format!("{}{}", self.template.revision_prefix, c.revision);
        match &c.number {
            Some(n) => format!("{n}  {rev}"),
            None => rev,
        }
    }

    /// The width of each column of a table, as fractions of the table that add up to one, when the template
    /// sizes columns from their text or the document gives a width. None keeps each writer's 0.1 widths.
    ///
    /// The rule: a column needs its longest word and would like its longest line (the header included),
    /// both counted in characters, plus two for the cell's padding. A column whose line fits in a fair share
    /// of the free width gets it; this repeats with what is left until no more fit. The others each get their
    /// longest word, and the rest of the width goes in proportion to what each still wants. A width the
    /// document gives is kept, and the other columns share what is left.
    pub fn column_widths(&self, t: &crate::resolve::RTable) -> Option<Vec<f64>> {
        let n = t.columns.len();
        if n == 0 || !(self.template.content_widths || t.columns.iter().any(|c| c.width.is_some())) {
            return None;
        }
        let words = |s: &str| s.split_whitespace().map(|w| w.chars().count()).max().unwrap_or(0) as f64;
        let mut min = vec![1.0f64; n];
        let mut max = vec![1.0f64; n];
        for (i, c) in t.columns.iter().enumerate() {
            let h = c.header();
            min[i] = min[i].max(words(&h));
            max[i] = max[i].max(h.chars().count() as f64);
        }
        for row in &t.rows {
            for (i, cell) in row.iter().enumerate().take(n) {
                // A line break splits a cell into lines; the longest line is what it would like.
                for line in
                    crate::resolve::cell_inlines(cell).split(|x| *x == crate::markdown::Inline::LineBreak)
                {
                    let text = crate::markdown::plain(line);
                    min[i] = min[i].max(words(&text));
                    max[i] = max[i].max(text.chars().count() as f64);
                }
            }
        }
        for i in 0..n {
            min[i] += 2.0;
            max[i] += 2.0;
        }
        // Characters across the text width, at about half an em each.
        let tw = f64::from(
            self.template.page_width_mm - self.template.margin_left_mm - self.template.margin_right_mm,
        );
        let em_mm = f64::from(self.template.body_size_pt) * 0.3528;
        let capacity = tw / (em_mm * 0.5);
        let fixed: f64 = t.columns.iter().filter_map(|c| c.width).sum::<f64>().min(1.0);
        let mut room = capacity * (1.0 - fixed);
        let mut want = vec![0.0f64; n];
        let mut open: Vec<usize> = (0..n).filter(|&i| t.columns[i].width.is_none()).collect();
        let free = open.clone();
        loop {
            if open.is_empty() {
                break;
            }
            let fair = room / open.len() as f64;
            let fits: Vec<usize> = open.iter().copied().filter(|&i| max[i] <= fair).collect();
            if fits.is_empty() {
                break;
            }
            for &i in &fits {
                want[i] = max[i];
                room -= max[i];
            }
            open.retain(|i| !fits.contains(i));
        }
        if !open.is_empty() {
            let total_min: f64 = open.iter().map(|&i| min[i]).sum();
            let spare = (room - total_min).max(0.0);
            let slack: f64 = open.iter().map(|&i| max[i] - min[i]).sum::<f64>().max(1e-9);
            for &i in &open {
                want[i] = min[i] + spare * (max[i] - min[i]) / slack;
            }
        }
        let sum_free: f64 = free.iter().map(|&i| want[i]).sum::<f64>().max(1e-9);
        let out: Vec<f64> = (0..n)
            .map(|i| match t.columns[i].width {
                Some(w) => w,
                None => (1.0 - fixed) * want[i] / sum_free,
            })
            .collect();
        // Rounded to a thousandth, so every writer prints the same numbers.
        Some(out.iter().map(|w| (w * 1000.0).round() / 1000.0).collect())
    }

    /// The left of the running foot: the document's `footer`, or else the organisation and the notice,
    /// joined by " · ".
    pub fn footer_left(&self) -> String {
        if let Some(f) = &self.doc.footer {
            return f.clone();
        }
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

    /// True when the running foot is the three-column foot of 0.2 (left text, middle, page number), which
    /// a document asks for with `footer` or `foot_sensitivity`. Otherwise it is the 0.1 foot, unchanged.
    pub fn grid_foot(&self) -> bool {
        self.doc.footer.is_some() || self.template.foot_sensitivity
    }

    /// The middle of the running foot: the sensitivity label in capitals, when the template asks for it.
    pub fn footer_middle(&self) -> Option<String> {
        self.template.foot_sensitivity.then(|| self.doc.control.sensitivity.to_uppercase())
    }

    /// A section heading as printed, in the template's number style.
    pub fn heading(&self, s: &crate::resolve::RSection) -> String {
        crate::resolve::heading_text_as(self.template.heading_numbers, s.number.as_deref(), &s.title)
    }

    /// A table-of-contents line as printed, in the template's number style.
    pub fn toc_text(&self, e: &crate::resolve::TocEntry) -> String {
        match e.number.as_deref() {
            Some(n) if n.starts_with("Appendix ") => crate::resolve::heading_text(Some(n), &e.title),
            n => crate::resolve::heading_text_as(self.template.heading_numbers, n, &e.title),
        }
    }

    /// The rows of the cover's control block: the document's `cover_details`, or else the number, revision,
    /// date and status. Empty leaves the block out.
    pub fn cover_rows(&self) -> Vec<(String, String)> {
        match &self.doc.cover_details {
            Some(d) => d.clone(),
            None => self.cover_control().into_iter().map(|(l, v)| (l.to_string(), v)).collect(),
        }
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

/// The title of the document-control page, and of the control block on a photo cover.
pub const CONTROL: &str = "Document control";

/// "Page 3 of 12": the right of the running foot. Writers that count pages fill the numbers themselves.
pub const PAGE_OF: (&str, &str) = ("Page ", " of ");
