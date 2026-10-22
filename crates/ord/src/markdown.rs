//! Prose (spec 0001 §6): Markdown read into a small model that all three writers share.
//!
//! The reader is CommonMark with no maths: a `$` is always a literal dollar sign. Headings, tables, images
//! and raw HTML are not part of the model; the reader reports them so the checks can refuse them (ORD012).
//! Cross-references `[[#id]]` become `Inline::XRef`, resolved later (`crate::resolve`).

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// A block of prose.
#[derive(Debug, Clone, PartialEq)]
pub enum MdBlock {
    Paragraph(Vec<Inline>),
    List { ordered: bool, start: u64, items: Vec<Vec<MdBlock>> },
    Quote(Vec<MdBlock>),
    Code(String),
}

/// A run of inline text.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Code(String),
    Link {
        url: String,
        children: Vec<Inline>,
    },
    /// `[[#id]]`. `text` and `anchor` are empty until the document is resolved.
    XRef {
        id: String,
        text: String,
        anchor: String,
    },
    LineBreak,
}

/// Something prose may not hold (ORD012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Heading,
    Table,
    Image,
    Html,
}

impl Refused {
    pub fn describe(&self) -> &'static str {
        match self {
            Refused::Heading => "a heading (use a section block)",
            Refused::Table => "a table (use a table block)",
            Refused::Image => "an image (use a figure block)",
            Refused::Html => "raw HTML",
        }
    }
}

/// Prose read from Markdown: its blocks, and anything it held that prose may not.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prose {
    pub blocks: Vec<MdBlock>,
    pub refused: Vec<Refused>,
}

/// The reader's options. Tables are on only so that a table can be found and refused; maths stays off.
fn options() -> Options {
    Options::ENABLE_TABLES
}

/// Reads Markdown prose.
pub fn read(markdown: &str) -> Prose {
    let mut r = Reader { refused: Vec::new() };
    let events: Vec<Event> = Parser::new_ext(markdown, options()).collect();
    let mut it = events.into_iter().peekable();
    let blocks = r.blocks(&mut it, None);
    Prose { blocks, refused: r.refused }
}

struct Reader {
    refused: Vec<Refused>,
}

type Events<'a> = std::iter::Peekable<std::vec::IntoIter<Event<'a>>>;

impl Reader {
    fn refuse(&mut self, what: Refused) {
        if !self.refused.contains(&what) {
            self.refused.push(what);
        }
    }

    /// Blocks up to the end tag `until` (or the end of input).
    fn blocks(&mut self, it: &mut Events, until: Option<TagEnd>) -> Vec<MdBlock> {
        let mut out = Vec::new();
        let mut loose: Vec<Inline> = Vec::new();
        while let Some(ev) = it.next() {
            match ev {
                Event::End(end) if Some(end) == until => break,
                Event::Start(Tag::Paragraph) => {
                    flush(&mut loose, &mut out);
                    let inl = self.inlines(it, TagEnd::Paragraph);
                    if !inl.is_empty() {
                        out.push(MdBlock::Paragraph(inl));
                    }
                }
                Event::Start(Tag::List(start)) => {
                    flush(&mut loose, &mut out);
                    let mut items = Vec::new();
                    loop {
                        match it.next() {
                            Some(Event::Start(Tag::Item)) => items.push(self.blocks(it, Some(TagEnd::Item))),
                            Some(Event::End(TagEnd::List(_))) | None => break,
                            Some(_) => {}
                        }
                    }
                    // An empty item is dropped.
                    items.retain(|i: &Vec<MdBlock>| !i.is_empty());
                    if !items.is_empty() {
                        out.push(MdBlock::List {
                            ordered: start.is_some(),
                            start: start.unwrap_or(1),
                            items,
                        });
                    }
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    flush(&mut loose, &mut out);
                    let inner = self.blocks(it, Some(TagEnd::BlockQuote(None)));
                    out.push(MdBlock::Quote(inner));
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    flush(&mut loose, &mut out);
                    let mut text = String::new();
                    for ev in it.by_ref() {
                        match ev {
                            Event::Text(t) => text.push_str(&t),
                            Event::End(TagEnd::CodeBlock) => break,
                            _ => {}
                        }
                    }
                    if text.ends_with('\n') {
                        text.pop();
                    }
                    out.push(MdBlock::Code(text));
                }
                Event::Start(Tag::Heading { .. }) => {
                    self.refuse(Refused::Heading);
                    // Keep the text, as a paragraph, so nothing is lost if a caller ignores the finding.
                    let inl = self.inlines(it, TagEnd::Heading(pulldown_cmark::HeadingLevel::H1));
                    flush(&mut loose, &mut out);
                    if !inl.is_empty() {
                        out.push(MdBlock::Paragraph(inl));
                    }
                }
                Event::Start(Tag::Table(_)) => {
                    self.refuse(Refused::Table);
                    self.skip_to_end(it);
                }
                Event::Start(Tag::HtmlBlock) => {
                    self.refuse(Refused::Html);
                    self.skip_to_end(it);
                }
                Event::Html(_) | Event::InlineHtml(_) => self.refuse(Refused::Html),
                // Tight list items hold their text without a paragraph.
                Event::Start(tag) if is_inline_start(&tag) => {
                    let inl = self.inline_from(tag, it);
                    loose.extend(inl);
                }
                Event::Text(t) => push_text(&mut loose, &t),
                Event::Code(c) => loose.push(Inline::Code(c.to_string())),
                Event::SoftBreak => push_text(&mut loose, " "),
                Event::HardBreak => loose.push(Inline::LineBreak),
                Event::Rule => {}
                _ => {}
            }
        }
        flush(&mut loose, &mut out);
        out
    }

    fn skip_to_end(&mut self, it: &mut Events) {
        let mut depth = 1usize;
        for ev in it.by_ref() {
            match ev {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    /// Inlines up to the end tag `until` (matched by kind, so any heading level ends a heading).
    fn inlines(&mut self, it: &mut Events, until: TagEnd) -> Vec<Inline> {
        let mut out = Vec::new();
        while let Some(ev) = it.next() {
            match ev {
                Event::End(end) if same_kind(&end, &until) => break,
                Event::Start(tag) if is_inline_start(&tag) => {
                    let inl = self.inline_from(tag, it);
                    out.extend(inl);
                }
                Event::Start(Tag::Image { .. }) => {
                    self.refuse(Refused::Image);
                    self.skip_to_end(it);
                }
                Event::Text(t) => push_text(&mut out, &t),
                Event::Code(c) => out.push(Inline::Code(c.to_string())),
                Event::SoftBreak => push_text(&mut out, " "),
                Event::HardBreak => out.push(Inline::LineBreak),
                Event::Html(_) | Event::InlineHtml(_) => self.refuse(Refused::Html),
                _ => {}
            }
        }
        split_xrefs(out)
    }

    fn inline_from(&mut self, tag: Tag, it: &mut Events) -> Vec<Inline> {
        match tag {
            Tag::Emphasis => vec![Inline::Emph(self.inlines(it, TagEnd::Emphasis))],
            Tag::Strong => vec![Inline::Strong(self.inlines(it, TagEnd::Strong))],
            Tag::Link { dest_url, .. } => {
                vec![Inline::Link { url: dest_url.to_string(), children: self.inlines(it, TagEnd::Link) }]
            }
            Tag::Image { .. } => {
                self.refuse(Refused::Image);
                self.skip_to_end(it);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

fn is_inline_start(tag: &Tag) -> bool {
    matches!(tag, Tag::Emphasis | Tag::Strong | Tag::Link { .. } | Tag::Image { .. })
}

fn same_kind(a: &TagEnd, b: &TagEnd) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn push_text(out: &mut Vec<Inline>, t: &str) {
    if let Some(Inline::Text(prev)) = out.last_mut() {
        prev.push_str(t);
    } else {
        out.push(Inline::Text(t.to_string()));
    }
}

fn flush(loose: &mut Vec<Inline>, out: &mut Vec<MdBlock>) {
    if !loose.is_empty() {
        let inl = split_xrefs(std::mem::take(loose));
        out.push(MdBlock::Paragraph(inl));
    }
}

/// Splits `[[#id]]` out of text runs. The reader leaves the brackets as text; adjacent text runs were merged
/// by `push_text`, so a reference is never split across two runs.
fn split_xrefs(inl: Vec<Inline>) -> Vec<Inline> {
    let mut out = Vec::new();
    for i in inl {
        match i {
            Inline::Text(t) => {
                let mut rest = t.as_str();
                while let Some(start) = rest.find("[[#") {
                    let after = &rest[start + 3..];
                    let Some(end) = after.find("]]") else { break };
                    let id = &after[..end];
                    if id.is_empty() || id.chars().any(|c| c.is_whitespace() || c == '[' || c == ']') {
                        push_text(&mut out, &rest[..start + 3]);
                        rest = after;
                        continue;
                    }
                    push_text(&mut out, &rest[..start]);
                    out.push(Inline::XRef { id: id.to_string(), text: String::new(), anchor: String::new() });
                    rest = &after[end + 2..];
                }
                if !rest.is_empty() {
                    push_text(&mut out, rest);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The plain text of inlines, as a reader would see it (cross-references by their resolved text).
pub fn plain(inl: &[Inline]) -> String {
    let mut s = String::new();
    for i in inl {
        match i {
            Inline::Text(t) | Inline::Code(t) => s.push_str(t),
            Inline::Emph(c) | Inline::Strong(c) | Inline::Link { children: c, .. } => s.push_str(&plain(c)),
            Inline::XRef { text, .. } => s.push_str(text),
            Inline::LineBreak => s.push(' '),
        }
    }
    s
}

/// Every text run of the blocks, in order: for the typed-reference check.
pub fn texts(blocks: &[MdBlock]) -> Vec<String> {
    fn inl(i: &[Inline], out: &mut Vec<String>) {
        for x in i {
            match x {
                Inline::Text(t) => out.push(t.clone()),
                Inline::Emph(c) | Inline::Strong(c) | Inline::Link { children: c, .. } => inl(c, out),
                _ => {}
            }
        }
    }
    fn blk(b: &[MdBlock], out: &mut Vec<String>) {
        for x in b {
            match x {
                MdBlock::Paragraph(i) => inl(i, out),
                MdBlock::List { items, .. } => items.iter().for_each(|it| blk(it, out)),
                MdBlock::Quote(q) => blk(q, out),
                MdBlock::Code(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    blk(blocks, &mut out);
    out
}

/// Applies `f` to every cross-reference in the blocks.
pub fn for_each_xref(blocks: &mut [MdBlock], f: &mut impl FnMut(&str, &mut String, &mut String)) {
    fn inl(i: &mut [Inline], f: &mut impl FnMut(&str, &mut String, &mut String)) {
        for x in i {
            match x {
                Inline::XRef { id, text, anchor } => f(id, text, anchor),
                Inline::Emph(c) | Inline::Strong(c) | Inline::Link { children: c, .. } => inl(c, f),
                _ => {}
            }
        }
    }
    for b in blocks {
        match b {
            MdBlock::Paragraph(i) => inl(i, f),
            MdBlock::List { items, .. } => items.iter_mut().for_each(|it| for_each_xref(it, f)),
            MdBlock::Quote(q) => for_each_xref(q, f),
            MdBlock::Code(_) => {}
        }
    }
}
