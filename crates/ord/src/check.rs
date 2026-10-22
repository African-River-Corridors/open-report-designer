//! The checks (spec 0001 §8). Any error refuses a render; warnings travel with the output.

use std::collections::BTreeSet;

use crate::assets::Assets;
use crate::finding::Finding;
use crate::model::Document;
use crate::register::{Register, Status};

/// Every check on a document, in a stable order.
pub fn check(doc: &Document, assets: &Assets, register: &Register) -> Vec<Finding> {
    let _ = assets;
    let mut out = Vec::new();
    control(doc, register, &mut out);
    out
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
