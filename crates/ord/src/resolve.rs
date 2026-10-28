//! Resolution (spec 0001 §5): the engine numbers everything once, and the writers print what they are given.
//!
//! The resolved document holds section numbers, appendix letters, table and figure numbers, anchors, the
//! prose read into its model with every cross-reference's text, and the table of contents. Resolution also
//! reports the structural findings (ORD009, ORD010, ORD012 to ORD020); `crate::check` gathers them.

use std::collections::BTreeMap;

use crate::finding::Finding;
use crate::markdown::{self, Inline, MdBlock};
use crate::model::{self, Align, Block, Cell, Document, HeadingNumbers, Sensitivity};
use crate::register::{Register, Stage};

/// The deepest heading level.
pub const MAX_LEVEL: u8 = 4;
/// The most appendices: one per letter.
pub const MAX_APPENDICES: usize = 26;

/// A document ready for the writers.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub title: String,
    pub subtitle: Option<String>,
    pub organisation: Option<String>,
    pub language: String,
    pub notice: Option<String>,
    pub summary: Option<Vec<MdBlock>>,
    pub control: ControlView,
    /// Whether to print a table of contents.
    pub contents: bool,
    pub toc: Vec<TocEntry>,
    pub body: Vec<RBlock>,
    pub appendices: Vec<RAppendix>,
    /// (0.2) The cover photograph: asset id and alt text.
    pub cover_image: Option<(String, String)>,
    /// (0.2) The cover's logo strip.
    pub cover_logos: Vec<model::CoverLogo>,
    /// (0.2) The rows of the cover's control block, when the document gives them.
    pub cover_details: Option<Vec<(String, String)>>,
    /// (0.2) The text on the left of the running foot.
    pub footer: Option<String>,
}

/// Document control as the writers print it.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlView {
    /// The class's display name, such as "Report".
    pub class_name: String,
    pub number: Option<String>,
    pub revision: String,
    pub date: String,
    /// The status's label, such as "For approval".
    pub status: String,
    /// Printed on every page while the status has furniture, such as "Draft".
    pub furniture: Option<String>,
    /// True when the status is issued (not a working status).
    pub issued: bool,
    pub sensitivity: String,
    pub prepared: Option<String>,
    pub checked: Option<String>,
    pub approved: Option<String>,
    pub history: Vec<model::HistoryEntry>,
    /// Every status of the lifecycle, normalised id and alias to label, for the history table.
    pub status_labels: BTreeMap<String, String>,
}

impl ControlView {
    /// The document-control rows, in order, as (label, value). Rows with no value are left out.
    pub fn rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = Vec::new();
        if let Some(n) = &self.number {
            rows.push(("Document number", n.clone()));
        }
        rows.push(("Revision", self.revision.clone()));
        rows.push(("Date", self.date.clone()));
        rows.push(("Status", self.status.clone()));
        rows.push(("Sensitivity", self.sensitivity.clone()));
        for (label, v) in
            [("Prepared", &self.prepared), ("Checked", &self.checked), ("Approved", &self.approved)]
        {
            if let Some(v) = v.as_ref().filter(|v| !v.trim().is_empty()) {
                rows.push((label, v.clone()));
            }
        }
        rows
    }

    /// The column titles of the revision history table.
    pub const HISTORY_COLUMNS: [&'static str; 4] = ["Revision", "Date", "Status", "Description"];
}

/// One line of the table of contents.
#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    /// 1 for a top-level section or an appendix.
    pub level: u8,
    /// "2.1", or "Appendix A". None for an unnumbered section.
    pub number: Option<String>,
    pub title: String,
    pub anchor: String,
}

/// A resolved block.
#[derive(Debug, Clone, PartialEq)]
pub enum RBlock {
    Section(RSection),
    Prose(Vec<MdBlock>),
    Table(RTable),
    Figure(RFigure),
    DataBox(RDataBox),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RSection {
    /// 1 to 4. A top-level body section is 1; a section directly in an appendix is 2.
    pub level: u8,
    /// "2.1", "A.1". None when unnumbered.
    pub number: Option<String>,
    pub title: String,
    pub anchor: String,
    pub body: Vec<RBlock>,
}

impl RSection {
    /// The heading as printed: "2.1  Scope", or the title alone.
    pub fn heading(&self) -> String {
        heading_text(self.number.as_deref(), &self.title)
    }
}

/// "2.1  Scope": the number and the title with two spaces, or the title alone.
pub fn heading_text(number: Option<&str>, title: &str) -> String {
    heading_text_as(HeadingNumbers::Spaced, number, title)
}

/// A heading in a template's number style: "2.1  Scope" (spaced), or "2. Scope" and "2.1 Scope" (dotted, where
/// a number with no point takes one).
pub fn heading_text_as(style: HeadingNumbers, number: Option<&str>, title: &str) -> String {
    match (number, style) {
        (Some(n), HeadingNumbers::Spaced) => format!("{n}  {title}"),
        (Some(n), HeadingNumbers::Dotted) if n.contains('.') => format!("{n} {title}"),
        (Some(n), HeadingNumbers::Dotted) => format!("{n}. {title}"),
        (None, _) => title.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RTable {
    /// "Table 3", "Table A.1". None for a table with no caption, which is not numbered.
    pub label: Option<String>,
    pub caption: Option<String>,
    pub anchor: String,
    /// False for a table with no header row.
    pub header: bool,
    /// The group row above the column titles: (title, span). Empty when there is none.
    pub groups: Vec<(String, u32)>,
    pub columns: Vec<RColumn>,
    /// Each cell is prose of at most one paragraph: a plain cell is one text run.
    pub rows: Vec<Vec<Vec<MdBlock>>>,
    pub notes: Option<Vec<MdBlock>>,
}

impl RTable {
    /// "Table 3: The options", or None for a table with no caption.
    pub fn caption_text(&self) -> Option<String> {
        Some(format!("{}: {}", self.label.as_ref()?, self.caption.as_ref()?))
    }

    /// For each column, the group it sits under: (index of the group, first column of the group). None when
    /// the table has no groups.
    pub fn group_of(&self, column: usize) -> Option<(usize, usize)> {
        let mut first = 0usize;
        for (g, (_, span)) in self.groups.iter().enumerate() {
            let end = first + *span as usize;
            if column < end {
                return Some((g, first));
            }
            first = end;
        }
        None
    }
}

/// The inlines of a resolved cell (empty for an empty cell).
pub fn cell_inlines(cell: &[MdBlock]) -> &[Inline] {
    match cell.first() {
        Some(MdBlock::Paragraph(i)) => i,
        _ => &[],
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RColumn {
    pub title: String,
    pub unit: Option<String>,
    pub align: Align,
    /// The column's share of the table width, when the document gives one.
    pub width: Option<f64>,
}

impl RColumn {
    /// The header as printed: "Length (km)", or the title alone.
    pub fn header(&self) -> String {
        match &self.unit {
            Some(u) => format!("{} ({u})", self.title),
            None => self.title.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RFigure {
    /// "Figure 2", "Figure A.1". Maps are numbered with the figures.
    pub label: String,
    pub caption: String,
    pub anchor: String,
    pub asset: String,
    pub alt: String,
    /// A fraction of the text width.
    pub width: f64,
    /// The key line of a map; None for a figure.
    pub map_key: Option<String>,
}

impl RFigure {
    /// "Figure 2: The reach".
    pub fn caption_text(&self) -> String {
        format!("{}: {}", self.label, self.caption)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RDataBox {
    pub title: String,
    pub anchor: String,
    pub items: Vec<(String, String)>,
    pub notes: Option<Vec<MdBlock>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RAppendix {
    /// "A".
    pub letter: String,
    pub title: String,
    pub anchor: String,
    pub body: Vec<RBlock>,
}

impl RAppendix {
    /// "Appendix A  Rule trace".
    pub fn heading(&self) -> String {
        format!("Appendix {}  {}", self.letter, self.title)
    }
}

/// What a cross-reference to an id prints, and where it points.
#[derive(Debug, Clone)]
struct Target {
    text: String,
    anchor: String,
}

/// Resolves a document. Never fails: problems are findings, and the result is still usable for checking.
pub fn resolve(doc: &Document, register: &Register) -> (Resolved, Vec<Finding>) {
    let mut r = Resolver::default();
    let mut body = Vec::new();
    let mut section_no: Vec<u32> = Vec::new();
    let mut counters = Counters::default();
    for (i, b) in doc.body.iter().enumerate() {
        if let Some(rb) =
            r.block(b, &format!("/body/{i}"), Scope::Body, 1, true, &mut section_no, &mut counters)
        {
            body.push(rb);
        }
    }
    let mut appendices = Vec::new();
    if doc.appendices.len() > MAX_APPENDICES {
        r.findings.push(Finding::error(
            "ORD015",
            format!("/appendices/{MAX_APPENDICES}"),
            format!("there are {} appendices; the most is {MAX_APPENDICES} (A to Z)", doc.appendices.len()),
        ));
    }
    for (i, a) in doc.appendices.iter().enumerate() {
        let path = format!("/appendices/{i}");
        let letter = letter(i);
        let anchor = r.anchor(a.id.as_deref(), &path, || format!("ord-appendix-{}", letter.to_lowercase()));
        if let Some(id) = &a.id {
            r.targets
                .insert(id.clone(), Target { text: format!("Appendix {letter}"), anchor: anchor.clone() });
        }
        r.title_check(&a.title, &format!("{path}/title"), true);
        if a.body.is_empty() {
            r.findings.push(Finding::warning("ORD018", format!("{path}/body"), "the appendix is empty"));
        }
        r.toc.push(TocEntry {
            level: 1,
            number: Some(format!("Appendix {letter}")),
            title: a.title.clone(),
            anchor: anchor.clone(),
        });
        let mut section_no: Vec<u32> = Vec::new();
        let mut counters = Counters::default();
        let mut abody = Vec::new();
        for (j, b) in a.body.iter().enumerate() {
            let scope = Scope::Appendix(letter.clone());
            if let Some(rb) =
                r.block(b, &format!("{path}/body/{j}"), scope, 2, true, &mut section_no, &mut counters)
            {
                abody.push(rb);
            }
        }
        appendices.push(RAppendix { letter, title: a.title.clone(), anchor, body: abody });
    }

    let mut summary = doc.summary.as_deref().map(|s| r.prose(s, "/summary"));

    // Cross-references, now that every target is known.
    let targets = std::mem::take(&mut r.targets);
    let mut unknown: Vec<(String, String)> = Vec::new();
    let mut fix = |blocks: &mut [MdBlock], path: &str| {
        markdown::for_each_xref(blocks, &mut |id, text, anchor| match targets.get(id) {
            Some(t) => {
                *text = t.text.clone();
                *anchor = t.anchor.clone();
            }
            None => {
                *text = format!("[{id}]");
                unknown.push((path.to_string(), id.to_string()));
            }
        });
    };
    if let Some(s) = summary.as_mut() {
        fix(s, "/summary");
    }
    for (path, blocks) in r.prose_paths(&mut body, &mut appendices) {
        fix(blocks, &path);
    }
    for (path, id) in unknown {
        r.findings.push(Finding::error(
            "ORD010",
            path,
            format!("the cross-reference [[#{id}]] names no block"),
        ));
    }

    let class = register.class(&doc.class);
    let status =
        class.and_then(|c| register.lifecycle(&c.lifecycle)).and_then(|l| l.status(&doc.control.status));
    let c = &doc.control;
    let control = ControlView {
        class_name: class.map(|c| c.name.clone()).unwrap_or_else(|| doc.class.clone()),
        number: c.number.clone(),
        revision: c.revision.clone(),
        date: c.date.clone(),
        status: status.map(|s| s.label.clone()).unwrap_or_else(|| c.status.clone()),
        furniture: status.and_then(|s| s.furniture.clone()),
        issued: status.is_some_and(|s| s.stage == Stage::Issued),
        sensitivity: c.sensitivity.unwrap_or(Sensitivity::Public).label().to_string(),
        prepared: c.prepared.clone(),
        checked: c.checked.clone(),
        approved: c.approved.clone(),
        history: c.history.clone(),
        status_labels: class
            .and_then(|c| register.lifecycle(&c.lifecycle))
            .map(|l| {
                let mut m: BTreeMap<String, String> =
                    l.statuses.iter().map(|s| (s.id.clone(), s.label.clone())).collect();
                for (alias, target) in &l.aliases {
                    if let Some(s) = l.status(target) {
                        m.insert(crate::register::normalise(alias), s.label.clone());
                    }
                }
                m
            })
            .unwrap_or_default(),
    };
    let contents = doc.contents.unwrap_or_else(|| class.is_some_and(|c| c.contents));
    let resolved = Resolved {
        title: doc.title.clone(),
        subtitle: doc.subtitle.clone(),
        organisation: doc.organisation.clone(),
        language: doc.language().to_string(),
        notice: doc.notice.clone(),
        summary,
        control,
        contents,
        toc: r.toc,
        body,
        appendices,
        cover_image: doc.cover_image.as_ref().map(|c| (c.asset.clone(), c.alt.clone())),
        cover_logos: doc.cover_logos.clone(),
        cover_details: doc
            .cover_details
            .as_ref()
            .map(|d| d.iter().map(|i| (i.label.clone(), i.value.clone())).collect()),
        footer: doc.footer.clone(),
    };
    (resolved, r.findings)
}

/// The letter of the i-th appendix (0 is A). Past Z it keeps counting (AA), but ORD015 refuses that.
pub fn letter(i: usize) -> String {
    let mut n = i;
    let mut s = Vec::new();
    loop {
        s.push((b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s.iter().rev().collect()
}

#[derive(Clone)]
enum Scope {
    Body,
    Appendix(String),
}

#[derive(Default)]
struct Counters {
    table: u32,
    figure: u32,
    data_box: u32,
}

#[derive(Default)]
struct Resolver {
    findings: Vec<Finding>,
    targets: BTreeMap<String, Target>,
    /// Every id seen, with the path of its first use.
    ids: BTreeMap<String, String>,
    toc: Vec<TocEntry>,
    /// Prose blocks in document order with their paths, for the cross-reference pass.
    prose_order: Vec<String>,
}

impl Resolver {
    /// The anchor of a block: its id, or a generated one. Records the id and refuses a repeat (ORD009).
    fn anchor(&mut self, id: Option<&str>, path: &str, generated: impl FnOnce() -> String) -> String {
        match id {
            Some(id) => {
                if let Some(first) = self.ids.get(id) {
                    self.findings.push(Finding::error(
                        "ORD009",
                        path,
                        format!("the id {id:?} is already used at {first}"),
                    ));
                } else {
                    self.ids.insert(id.to_string(), path.to_string());
                }
                id.to_string()
            }
            None => generated(),
        }
    }

    fn prose(&mut self, md: &str, path: &str) -> Vec<MdBlock> {
        let p = markdown::read(md);
        for what in &p.refused {
            self.findings.push(Finding::error(
                "ORD012",
                path,
                format!("prose may not hold {}", what.describe()),
            ));
        }
        for t in markdown::texts(&p.blocks) {
            if let Some(m) = typed_reference(&t) {
                self.findings.push(Finding::warning(
                    "ORD017",
                    path,
                    format!("{m:?} is a typed reference; write [[#id]] so the number follows the block"),
                ));
            }
        }
        p.blocks
    }

    /// A table cell: a plain cell is one text run; a Markdown cell is read as prose and must be one paragraph
    /// (ORD012 otherwise).
    fn cell(&mut self, cell: &Cell, path: &str) -> Vec<MdBlock> {
        match cell {
            Cell::Text(t) if t.is_empty() => Vec::new(),
            Cell::Text(t) => vec![MdBlock::Paragraph(vec![Inline::Text(t.clone())])],
            Cell::Markdown(m) => {
                let blocks = self.prose(&m.markdown, &format!("{path}/markdown"));
                match blocks.as_slice() {
                    [] | [MdBlock::Paragraph(_)] => blocks,
                    _ => {
                        self.findings.push(Finding::error(
                            "ORD012",
                            format!("{path}/markdown"),
                            "a table cell holds one paragraph of inline Markdown: no lists, quotes, code blocks \
                             or second paragraph (use a hard line break)",
                        ));
                        Vec::new()
                    }
                }
            }
        }
    }

    fn title_check(&mut self, title: &str, path: &str, appendix: bool) {
        if typed_number(title, appendix) {
            self.findings.push(Finding::error(
                "ORD016",
                path,
                format!("the title {title:?} starts with a typed number; the engine numbers headings"),
            ));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn block(
        &mut self,
        b: &Block,
        path: &str,
        scope: Scope,
        level: u8,
        numbered: bool,
        section_no: &mut Vec<u32>,
        counters: &mut Counters,
    ) -> Option<RBlock> {
        let prefix = match &scope {
            Scope::Body => String::new(),
            Scope::Appendix(l) => format!("{l}."),
        };
        Some(match b {
            Block::Section(s) => {
                if level > MAX_LEVEL {
                    self.findings.push(Finding::error(
                        "ORD015",
                        path,
                        format!("sections nest {level} levels deep here; the most is {MAX_LEVEL}"),
                    ));
                }
                self.title_check(&s.title, &format!("{path}/title"), false);
                if s.body.is_empty() {
                    self.findings.push(Finding::warning(
                        "ORD018",
                        format!("{path}/body"),
                        "the section is empty",
                    ));
                }
                let numbered = numbered && s.numbered;
                // The depth of this section among numbered sections in its scope.
                let depth = (level - if matches!(scope, Scope::Body) { 1 } else { 2 }) as usize;
                let number = if numbered {
                    section_no.truncate(depth + 1);
                    while section_no.len() <= depth {
                        section_no.push(0);
                    }
                    section_no[depth] += 1;
                    let parts: Vec<String> = section_no.iter().map(u32::to_string).collect();
                    Some(format!("{prefix}{}", parts.join(".")))
                } else {
                    None
                };
                let anchor = self.anchor(s.id.as_deref(), path, || match &number {
                    Some(n) => format!("ord-section-{}", n.to_lowercase().replace('.', "-")),
                    None => format!("ord-section{}", path.replace('/', "-")),
                });
                if let Some(id) = &s.id {
                    let text = match &number {
                        Some(n) => format!("Section {n}"),
                        None => s.title.clone(),
                    };
                    self.targets.insert(id.clone(), Target { text, anchor: anchor.clone() });
                }
                if matches!(scope, Scope::Body) && level <= 3 {
                    self.toc.push(TocEntry {
                        level,
                        number: number.clone(),
                        title: s.title.clone(),
                        anchor: anchor.clone(),
                    });
                }
                let mut body = Vec::new();
                let mut child_no = section_no.clone();
                for (i, c) in s.body.iter().enumerate() {
                    let child_path = format!("{path}/body/{i}");
                    if let Some(rb) = self.block(
                        c,
                        &child_path,
                        scope.clone(),
                        level + 1,
                        numbered,
                        &mut child_no,
                        counters,
                    ) {
                        body.push(rb);
                    }
                }
                RBlock::Section(RSection { level, number, title: s.title.clone(), anchor, body })
            }
            Block::Prose(p) => {
                self.prose_order.push(format!("{path}/markdown"));
                RBlock::Prose(self.prose(&p.markdown, &format!("{path}/markdown")))
            }
            Block::Table(t) => {
                // A table with no caption is not numbered: a reference to it prints "the table".
                let label = t.caption.as_ref().map(|_| {
                    counters.table += 1;
                    format!("Table {prefix}{}", counters.table)
                });
                let n = counters.table;
                let anchor = self.anchor(t.id.as_deref(), path, || match &label {
                    Some(l) => format!("ord-table-{}", l[6..].to_lowercase().replace('.', "-")),
                    None => format!("ord-table{}-{n}", path.replace('/', "-")),
                });
                if let Some(id) = &t.id {
                    let text = label.clone().unwrap_or_else(|| "the table".into());
                    self.targets.insert(id.clone(), Target { text, anchor: anchor.clone() });
                }
                for (i, c) in t.columns.iter().enumerate() {
                    if let Some(w) = c.width
                        && !(w > 0.0 && w <= 1.0)
                    {
                        self.findings.push(Finding::error(
                            "ORD020",
                            format!("{path}/columns/{i}/width"),
                            format!("the width {w} is outside 0 < w <= 1"),
                        ));
                    }
                }
                let given: f64 = t.columns.iter().filter_map(|c| c.width).sum();
                if given > 1.0 + 1e-9 {
                    self.findings.push(Finding::error(
                        "ORD020",
                        format!("{path}/columns"),
                        format!("the column widths add up to {given}, more than the table"),
                    ));
                }
                if !t.column_groups.is_empty() {
                    let total: u64 = t.column_groups.iter().map(|g| u64::from(g.span)).sum();
                    if t.column_groups.iter().any(|g| g.span == 0) || total != t.columns.len() as u64 {
                        self.findings.push(Finding::error(
                            "ORD013",
                            format!("{path}/column_groups"),
                            format!(
                                "the column groups span {total} columns, each at least one; the table has {}",
                                t.columns.len()
                            ),
                        ));
                    }
                }
                if t.rows.is_empty() {
                    self.findings.push(Finding::warning(
                        "ORD019",
                        format!("{path}/rows"),
                        "the table has no rows",
                    ));
                }
                for (i, row) in t.rows.iter().enumerate() {
                    if row.len() != t.columns.len() {
                        self.findings.push(Finding::error(
                            "ORD013",
                            format!("{path}/rows/{i}"),
                            format!(
                                "the row has {} cells; the table has {} columns",
                                row.len(),
                                t.columns.len()
                            ),
                        ));
                    }
                }
                let mut rows = Vec::with_capacity(t.rows.len());
                for (i, row) in t.rows.iter().enumerate() {
                    let mut cells = Vec::with_capacity(row.len());
                    for (j, cell) in row.iter().enumerate() {
                        let cpath = format!("{path}/rows/{i}/{j}");
                        self.prose_order.push(cpath.clone());
                        cells.push(self.cell(cell, &cpath));
                    }
                    rows.push(cells);
                }
                let notes = t.notes.as_deref().map(|n| {
                    self.prose_order.push(format!("{path}/notes"));
                    self.prose(n, &format!("{path}/notes"))
                });
                RBlock::Table(RTable {
                    label,
                    caption: t.caption.clone(),
                    anchor,
                    header: t.header.unwrap_or(true),
                    groups: t.column_groups.iter().map(|g| (g.title.clone(), g.span)).collect(),
                    columns: t
                        .columns
                        .iter()
                        .map(|c| RColumn {
                            title: c.title.clone(),
                            unit: c.unit.clone(),
                            align: c.align.unwrap_or_default(),
                            width: c.width,
                        })
                        .collect(),
                    rows,
                    notes,
                })
            }
            Block::Figure(f) => self.figure(
                path,
                &prefix,
                counters,
                f.id.as_deref(),
                &f.caption,
                &f.asset,
                &f.alt,
                f.width,
                None,
            ),
            Block::Map(m) => {
                let mut key = Vec::new();
                if let Some(c) = &m.crs {
                    key.push(format!("Coordinate system: {c}"));
                }
                if let Some(s) = &m.scale {
                    key.push(format!("Scale: {s}"));
                }
                key.push(format!("Source: {}", m.attribution));
                let key = key.join(". ") + ".";
                self.figure(
                    path,
                    &prefix,
                    counters,
                    m.id.as_deref(),
                    &m.caption,
                    &m.asset,
                    &m.alt,
                    m.width,
                    Some(key),
                )
            }
            Block::DataBox(d) => {
                counters.data_box += 1;
                let n = counters.data_box;
                let anchor = self.anchor(d.id.as_deref(), path, || {
                    format!("ord-box-{}{n}", prefix.to_lowercase().replace('.', "-"))
                });
                if let Some(id) = &d.id {
                    self.targets
                        .insert(id.clone(), Target { text: d.title().to_string(), anchor: anchor.clone() });
                }
                let notes = d.notes.as_deref().map(|n| {
                    self.prose_order.push(format!("{path}/notes"));
                    self.prose(n, &format!("{path}/notes"))
                });
                RBlock::DataBox(RDataBox {
                    title: d.title().to_string(),
                    anchor,
                    items: d.items.iter().map(|i| (i.label.clone(), i.value.clone())).collect(),
                    notes,
                })
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn figure(
        &mut self,
        path: &str,
        prefix: &str,
        counters: &mut Counters,
        id: Option<&str>,
        caption: &str,
        asset: &str,
        alt: &str,
        width: Option<f64>,
        map_key: Option<String>,
    ) -> RBlock {
        counters.figure += 1;
        let label = format!("Figure {prefix}{}", counters.figure);
        let anchor =
            self.anchor(id, path, || format!("ord-figure-{}", label[7..].to_lowercase().replace('.', "-")));
        if let Some(id) = id {
            self.targets.insert(id.to_string(), Target { text: label.clone(), anchor: anchor.clone() });
        }
        if alt.trim().is_empty() {
            self.findings.push(Finding::error(
                "ORD014",
                format!("{path}/alt"),
                "the alt text is empty; say what the image shows",
            ));
        }
        let w = width.unwrap_or(1.0);
        if !(w > 0.0 && w <= 1.0) {
            self.findings.push(Finding::error(
                "ORD020",
                format!("{path}/width"),
                format!("the width {w} is outside 0 < width <= 1"),
            ));
        }
        RBlock::Figure(RFigure {
            label,
            caption: caption.to_string(),
            anchor,
            asset: asset.to_string(),
            alt: alt.to_string(),
            width: w.clamp(0.05, 1.0),
            map_key,
        })
    }

    /// The prose blocks of the resolved tree with their paths, in document order.
    fn prose_paths<'a>(
        &self,
        body: &'a mut [RBlock],
        appendices: &'a mut [RAppendix],
    ) -> Vec<(String, &'a mut Vec<MdBlock>)> {
        fn walk<'a>(blocks: &'a mut [RBlock], out: &mut Vec<&'a mut Vec<MdBlock>>) {
            for b in blocks {
                match b {
                    RBlock::Section(s) => walk(&mut s.body, out),
                    RBlock::Prose(p) => out.push(p),
                    RBlock::Table(t) => {
                        for row in t.rows.iter_mut() {
                            for cell in row.iter_mut() {
                                out.push(cell);
                            }
                        }
                        if let Some(n) = t.notes.as_mut() {
                            out.push(n)
                        }
                    }
                    RBlock::DataBox(d) => {
                        if let Some(n) = d.notes.as_mut() {
                            out.push(n)
                        }
                    }
                    RBlock::Figure(_) => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(body, &mut out);
        for a in appendices {
            walk(&mut a.body, &mut out);
        }
        self.prose_order.iter().cloned().zip(out).collect()
    }
}

/// True when a title starts with a typed number: "2 Scope", "2.1 Scope", "1. Scope", "§2 Scope", or, for an
/// appendix, "A. Trace", "A.1 Trace", "A: Trace" or "Appendix A Trace".
fn typed_number(title: &str, appendix: bool) -> bool {
    let t = title.trim_start();
    if t.starts_with('§') {
        return true;
    }
    let digits = |s: &str| -> Option<usize> {
        // `\d{1,2}(\.\d{1,2})*\.?` followed by a space; returns the length of the number.
        let b = s.as_bytes();
        let mut i = 0;
        loop {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i == start || i - start > 2 {
                return None;
            }
            if i < b.len() && b[i] == b'.' {
                i += 1;
                if i < b.len() && b[i].is_ascii_digit() {
                    continue;
                }
            }
            return (i < b.len() && b[i] == b' ').then_some(i);
        }
    };
    if digits(t).is_some() {
        return true;
    }
    if appendix {
        let lower = t.to_lowercase();
        if lower.starts_with("appendix ") {
            return true;
        }
        let b = t.as_bytes();
        if b.len() > 2 && b[0].is_ascii_uppercase() {
            match b[1] {
                b'.' | b':' if b[2] == b' ' => return true,
                b'.' => return digits(&t[2..]).is_some(),
                _ => {}
            }
        }
    }
    false
}

/// A typed section reference in prose: "§3", "Section 3.2", "Appendix B", "Table 4", "Figure 2".
fn typed_reference(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        if let Some(rest) = w.strip_prefix('§')
            && (rest.chars().next().is_some_and(|c| c.is_ascii_digit())
                || (rest.is_empty()
                    && words.get(i + 1).is_some_and(|n| n.starts_with(|c: char| c.is_ascii_digit()))))
        {
            return Some(if rest.is_empty() { format!("§ {}", words[i + 1]) } else { w.to_string() });
        }
        let kw = matches!(*w, "Section" | "section" | "Sections" | "Table" | "Figure" | "Appendix" | "Annex");
        if let (true, Some(next)) = (kw, words.get(i + 1)) {
            let n = next.trim_end_matches([',', '.', ';', ':', ')']);
            let numeric = n.starts_with(|c: char| c.is_ascii_digit())
                && n.chars().all(|c| c.is_ascii_digit() || c == '.');
            let lettered = *w == "Appendix" || *w == "Annex";
            let letter = lettered && n.len() == 1 && n.chars().all(|c| c.is_ascii_uppercase());
            let lettered_number = n.len() > 2
                && n.as_bytes()[0].is_ascii_uppercase()
                && n.as_bytes()[1] == b'.'
                && n[2..].chars().all(|c| c.is_ascii_digit() || c == '.');
            if numeric || letter || lettered_number {
                return Some(format!("{w} {n}"));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_numbers_in_titles() {
        for t in ["2 Scope", "2.1 Scope", "1. Scope", "§2 Scope", "12.3.4 Deep"] {
            assert!(typed_number(t, false), "{t}");
        }
        for t in ["Scope", "3D model", "A note on scope", "2026 survey", "Section", "1.5m depth"] {
            assert!(!typed_number(t, false), "{t}");
        }
        for t in ["A. Trace", "A: Trace", "A.1 Trace", "Appendix A Trace", "appendix B: runs"] {
            assert!(typed_number(t, true), "{t}");
        }
        for t in ["A note on runs", "Rule trace", "AB testing"] {
            assert!(!typed_number(t, true), "{t}");
        }
    }

    #[test]
    fn typed_references_in_prose() {
        for t in [
            "see §3",
            "see § 3.2",
            "in Section 3.2.",
            "Table 4 lists",
            "Figure 2, left",
            "Appendix B",
            "Table A.1",
        ] {
            assert!(typed_reference(t).is_some(), "{t}");
        }
        for t in ["the section below", "Table of results", "Appendix material", "a figure of 2 m", "§"] {
            assert!(typed_reference(t).is_none(), "{t}");
        }
    }

    #[test]
    fn appendix_letters() {
        assert_eq!([letter(0), letter(1), letter(25), letter(26)], ["A", "B", "Z", "AA"]);
    }
}
