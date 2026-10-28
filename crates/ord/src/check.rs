//! The checks (spec 0001 §8). Any error refuses a render; warnings travel with the output.

use std::collections::BTreeSet;

use crate::assets::Assets;
use crate::finding::Finding;
use crate::model::{Block, Document};
use crate::register::{Register, Status};

/// Every check on a document, in a stable order.
pub fn check(doc: &Document, assets: &Assets, register: &Register) -> Vec<Finding> {
    let mut out = Vec::new();
    version(doc, &mut out);
    control(doc, register, &mut out);
    let (_, structural) = crate::resolve::resolve(doc, register);
    out.extend(structural);
    images(doc, assets, &mut out);
    cover(doc, assets, &mut out);
    out
}

/// A "0.1" document may not use a field added in "0.2" (ORD001), so that it still renders the same on a
/// 0.1 engine. The finding names the field and the version it needs.
fn version(doc: &Document, out: &mut Vec<Finding>) {
    if doc.ord != "0.1" {
        return;
    }
    let mut used: Vec<String> = Vec::new();
    if doc.cover_image.is_some() {
        used.push("/cover_image".into());
    }
    if !doc.cover_logos.is_empty() {
        used.push("/cover_logos".into());
    }
    if doc.cover_details.is_some() {
        used.push("/cover_details".into());
    }
    if doc.footer.is_some() {
        used.push("/footer".into());
    }
    for (id, a) in &doc.assets {
        if matches!(a.media_type, crate::model::MediaType::Jpeg | crate::model::MediaType::Font) {
            used.push(format!("/assets/{}/media_type", id.replace('~', "~0").replace('/', "~1")));
        }
    }
    if let Some(t) = &doc.template {
        let v = serde_json::to_value(t).unwrap_or_default();
        const V01: [&str; 7] =
            ["organisation_logo", "paper", "ink", "ink_2", "rule", "accent_ground", "cover"];
        if let Some(m) = v.as_object() {
            for k in m.keys().filter(|k| !V01.contains(&k.as_str())) {
                used.push(format!("/template/{k}"));
            }
        }
    }
    fn walk(blocks: &[Block], path: &str, used: &mut Vec<String>) {
        for (i, b) in blocks.iter().enumerate() {
            let p = format!("{path}/{i}");
            match b {
                Block::Section(s) => walk(&s.body, &format!("{p}/body"), used),
                Block::Table(t) => {
                    if t.caption.is_none() {
                        used.push(format!("{p}/caption"));
                    }
                    if t.header.is_some() {
                        used.push(format!("{p}/header"));
                    }
                    if !t.column_groups.is_empty() {
                        used.push(format!("{p}/column_groups"));
                    }
                    for (c, col) in t.columns.iter().enumerate() {
                        if col.width.is_some() {
                            used.push(format!("{p}/columns/{c}/width"));
                        }
                    }
                    for (r, row) in t.rows.iter().enumerate() {
                        for (c, cell) in row.iter().enumerate() {
                            if matches!(cell, crate::model::Cell::Markdown(_)) {
                                used.push(format!("{p}/rows/{r}/{c}"));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    walk(&doc.body, "/body", &mut used);
    for (i, a) in doc.appendices.iter().enumerate() {
        walk(&a.body, &format!("/appendices/{i}/body"), &mut used);
    }
    for path in used {
        out.push(Finding::error(
            "ORD001",
            &path,
            "this is a format 0.2 field (or value); the document says \"ord\": \"0.1\". Write \"0.2\"",
        ));
    }
}

/// The cover photograph and the logos name image assets that exist and are valid (ORD011); the photograph
/// has alt text (ORD014).
fn cover(doc: &Document, assets: &Assets, out: &mut Vec<Finding>) {
    if let Some(c) = &doc.cover_image {
        image(&c.asset, "/cover_image/asset", assets, out);
        if c.alt.trim().is_empty() {
            out.push(Finding::error("ORD014", "/cover_image/alt", "the cover photograph has no alt text"));
        }
    }
    for (i, l) in doc.cover_logos.iter().enumerate() {
        image(&l.asset, &format!("/cover_logos/{i}/asset"), assets, out);
    }
    let fonts = doc.template.as_ref().map(|t| t.fonts.as_slice()).unwrap_or_default();
    for (i, id) in fonts.iter().enumerate() {
        if let Some(a) = assets.get(id)
            && a.media_type == crate::model::MediaType::Font
            && let Err(why) = crate::assets::validate(a)
        {
            out.push(Finding::error(
                "ORD011",
                format!("/template/fonts/{i}"),
                format!("asset {id:?}: {why}"),
            ));
        }
    }
}

/// Every figure and map names an asset that exists, has a supported type and is a valid image (ORD011).
fn images(doc: &Document, assets: &Assets, out: &mut Vec<Finding>) {
    fn walk(blocks: &[Block], path: &str, assets: &Assets, out: &mut Vec<Finding>) {
        for (i, b) in blocks.iter().enumerate() {
            let p = format!("{path}/{i}");
            match b {
                Block::Section(s) => walk(&s.body, &format!("{p}/body"), assets, out),
                Block::Figure(f) => image(&f.asset, &format!("{p}/asset"), assets, out),
                Block::Map(m) => image(&m.asset, &format!("{p}/asset"), assets, out),
                _ => {}
            }
        }
    }
    walk(&doc.body, "/body", assets, out);
    for (i, a) in doc.appendices.iter().enumerate() {
        walk(&a.body, &format!("/appendices/{i}/body"), assets, out);
    }
}

fn image(id: &str, path: &str, assets: &Assets, out: &mut Vec<Finding>) {
    let Some(a) = assets.get(id) else {
        out.push(Finding::error("ORD011", path, format!("no asset {id:?}")));
        return;
    };
    if !a.media_type.is_image() {
        out.push(Finding::error("ORD011", path, format!("asset {id:?} is a font, not an image")));
        return;
    }
    if let Err(why) = crate::assets::validate(a) {
        out.push(Finding::error(
            "ORD011",
            path,
            format!("asset {id:?} is not a valid {}: {why}", a.media_type.as_str()),
        ));
    }
}

/// The document's class and status, resolved against the register. None when either is unknown.
pub fn resolve_status<'r>(doc: &Document, register: &'r Register) -> Option<&'r Status> {
    let class = register.class(&doc.class)?;
    register.lifecycle(&class.lifecycle)?.status(&doc.control.status)
}

/// Document control: class, status, sign-offs, revision form, number, dates and history (ORD002 to ORD008).
fn control(doc: &Document, register: &Register, out: &mut Vec<Finding>) {
    let c = &doc.control;
    if !is_iso_date(&c.date) {
        out.push(Finding::error(
            "ORD008",
            "/control/date",
            format!("{:?} is not an ISO 8601 date (YYYY-MM-DD)", c.date),
        ));
    }
    for (i, h) in c.history.iter().enumerate() {
        if !is_iso_date(&h.date) {
            out.push(Finding::error(
                "ORD008",
                format!("/control/history/{i}/date"),
                format!("{:?} is not an ISO 8601 date (YYYY-MM-DD)", h.date),
            ));
        }
    }

    let Some(class) = register.class(&doc.class) else {
        out.push(Finding::error(
            "ORD002",
            "/class",
            format!(
                "unknown document class {:?}; the register holds: {}. Add a class to the register rather than \
                 bending another one",
                doc.class,
                register.class_ids().join(", ")
            ),
        ));
        return;
    };
    let Some(lifecycle) = register.lifecycle(&class.lifecycle) else {
        out.push(Finding::error("ORD002", "/class", format!("class {:?} names no lifecycle", class.id)));
        return;
    };

    if let (Some(prefix), Some(number)) = (&class.number_prefix, &c.number)
        && !number.starts_with(&format!("{prefix}-"))
    {
        out.push(Finding::error(
            "ORD006",
            "/control/number",
            format!("a {} number starts with \"{prefix}-\"; {number:?} does not", class.name.to_lowercase()),
        ));
    }

    let Some(status) = lifecycle.status(&c.status) else {
        out.push(Finding::error(
            "ORD003",
            "/control/status",
            format!(
                "unknown status {:?} for a {}; the {} lifecycle allows: {}",
                c.status,
                class.name.to_lowercase(),
                lifecycle.id,
                lifecycle.status_ids().join(", ")
            ),
        ));
        return;
    };

    let missing: Vec<&str> = status
        .needs
        .iter()
        .filter(|s| {
            let v = match s {
                crate::register::SignOff::Prepared => &c.prepared,
                crate::register::SignOff::Checked => &c.checked,
                crate::register::SignOff::Approved => &c.approved,
            };
            v.as_deref().is_none_or(|v| v.trim().is_empty())
        })
        .map(|s| s.field())
        .collect();
    if !missing.is_empty() {
        let needs: Vec<&str> = status.needs.iter().map(|s| s.field()).collect();
        out.push(Finding::error(
            "ORD004",
            "/control",
            format!(
                "the status {:?} says the document was {}; it does not name who: {} missing",
                status.label,
                needs.join(", "),
                missing.join(", ")
            ),
        ));
    }

    if !status.revision.fits(&c.revision) {
        out.push(Finding::error(
            "ORD005",
            "/control/revision",
            format!(
                "the status {:?} needs {}, not {:?}",
                status.label,
                status.revision.describe(),
                c.revision
            ),
        ));
    }

    history(c, lifecycle, status, out);
}

fn history(
    c: &crate::model::Control,
    lifecycle: &crate::register::Lifecycle,
    status: &Status,
    out: &mut Vec<Finding>,
) {
    if c.history.is_empty() {
        return;
    }
    let mut seen = BTreeSet::new();
    let mut last_date: Option<&str> = None;
    for (i, h) in c.history.iter().enumerate() {
        let path = format!("/control/history/{i}");
        if !seen.insert(h.revision.as_str()) {
            out.push(Finding::error(
                "ORD007",
                format!("{path}/revision"),
                format!("revision {:?} is listed twice", h.revision),
            ));
        }
        if lifecycle.status(&h.status).is_none() {
            out.push(Finding::error(
                "ORD003",
                format!("{path}/status"),
                format!(
                    "unknown status {:?}; the {} lifecycle allows: {}",
                    h.status,
                    lifecycle.id,
                    lifecycle.status_ids().join(", ")
                ),
            ));
        }
        if is_iso_date(&h.date) {
            if let Some(prev) = last_date
                && h.date.as_str() < prev
            {
                out.push(Finding::error(
                    "ORD007",
                    format!("{path}/date"),
                    format!("the history goes back in time: {} comes after {prev}", h.date),
                ));
            }
            last_date = Some(&h.date);
        }
    }
    let last = c.history.last().expect("not empty");
    let last_status = lifecycle.status(&last.status).map(|s| s.id.as_str());
    if last.revision != c.revision || last.date != c.date || last_status != Some(status.id.as_str()) {
        out.push(Finding::error(
            "ORD007",
            format!("/control/history/{}", c.history.len() - 1),
            format!(
                "the last history entry ({}, {}, {}) is not the current revision ({}, {}, {})",
                last.revision, last.date, last.status, c.revision, c.date, c.status
            ),
        ));
    }
}

/// `YYYY-MM-DD` with a real month and day (leap years counted).
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = &s[r];
        part.bytes().all(|c| c.is_ascii_digit()).then(|| part.parse().ok()).flatten()
    };
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}
