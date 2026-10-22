//! The register, the lifecycle and the document-control checks (spec 0001 §4, §8 ORD002 to ORD008).
//! Label: property. No pinned numbers.

use ord::register::{RevisionForm, normalise};

fn doc(status: &str, revision: &str) -> serde_json::Value {
    serde_json::json!({
        "ord": "0.1", "class": "report", "title": "T",
        "control": { "number": "RPT-1", "revision": revision, "date": "2026-02-28", "status": status,
                     "prepared": "P", "checked": "C", "approved": "A" },
        "body": [ { "type": "prose", "markdown": "Hello." } ]
    })
}

fn findings_with(v: &serde_json::Value, reg: &ord::Register) -> Vec<ord::Finding> {
    let d = ord::parse(&v.to_string()).unwrap();
    ord::check(&d, &Default::default(), reg)
}

fn findings(v: &serde_json::Value) -> Vec<ord::Finding> {
    findings_with(v, &ord::builtin_register())
}

fn codes(v: &serde_json::Value) -> Vec<&'static str> {
    findings(v).into_iter().map(|f| f.code).collect()
}

#[test]
fn a_complete_issued_document_raises_nothing() {
    assert_eq!(codes(&doc("issued", "0")), Vec::<&str>::new());
}

#[test]
fn the_builtin_register_holds_four_classes_on_one_lifecycle() {
    let reg = ord::builtin_register();
    assert_eq!(reg.class_ids(), ["report", "technical-note", "schedule", "run-report"]);
    let prefixes: Vec<_> = reg.classes.iter().map(|c| c.number_prefix.clone().unwrap()).collect();
    assert_eq!(prefixes, ["RPT", "TN", "SCH", "RUN"]);
    assert_eq!(reg.classes.iter().filter(|c| c.contents).count(), 1);
    let ids = reg.lifecycle("standard").unwrap().status_ids();
    assert_eq!(ids, ["draft", "for-checking", "for-approval", "approved", "issued", "superseded"]);
}

#[test]
fn class_lookup_ignores_case_and_spacing_and_reads_aliases() {
    let reg = ord::builtin_register();
    assert_eq!(reg.class("  Report ").unwrap().id, "report");
    assert_eq!(reg.class("Technical_Note").unwrap().id, "technical-note");
    assert_eq!(reg.class("tech note").unwrap().id, "technical-note");
    assert!(reg.class("memo").is_none());
}

#[test]
fn an_unknown_class_is_ord002_and_lists_the_register() {
    let mut v = doc("issued", "0");
    v["class"] = "memo".into();
    let f = findings(&v);
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD002", "/class"));
    assert!(f[0].message.contains("report, technical-note, schedule, run-report"), "{}", f[0].message);
}

#[test]
fn status_aliases_resolve() {
    let reg = ord::builtin_register();
    let l = reg.lifecycle("standard").unwrap();
    for (name, id) in [
        ("For Approval", "for-approval"),
        ("for_approval", "for-approval"),
        ("for-sign-off", "for-approval"),
        ("for review", "for-checking"),
        ("FINAL", "issued"),
        ("draft", "draft"),
    ] {
        assert_eq!(l.status(name).unwrap().id, id, "{name}");
    }
    assert_eq!(normalise("  For   approval_"), "for-approval");
}

#[test]
fn an_unknown_status_is_ord003_and_lists_the_statuses() {
    let f = findings(&doc("published", "0"));
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD003", "/control/status"));
    assert!(f[0].message.contains("draft, for-checking, for-approval, approved, issued, superseded"));
}

/// Each status's gate refuses each missing sign-off, and the message names the status and what is missing.
#[test]
fn every_sign_off_a_status_needs_is_required() {
    let cases: &[(&str, &str, &[&str])] = &[
        ("draft", "A", &[]),
        ("for-checking", "A", &["prepared"]),
        ("for-approval", "A", &["prepared", "checked"]),
        ("approved", "0", &["prepared", "checked", "approved"]),
        ("issued", "0", &["prepared", "checked", "approved"]),
        ("superseded", "0", &["prepared", "checked", "approved"]),
    ];
    for (status, rev, needs) in cases {
        let ok = doc(status, rev);
        assert!(codes(&ok).is_empty(), "{status}: {:?}", findings(&ok));
        for field in ["prepared", "checked", "approved"] {
            let mut v = doc(status, rev);
            v["control"].as_object_mut().unwrap().remove(field);
            let f = findings(&v);
            if needs.contains(&field) {
                assert_eq!(f.len(), 1, "{status} without {field}: {f:?}");
                assert_eq!((f[0].code, f[0].path.as_str()), ("ORD004", "/control"));
                assert!(f[0].message.contains(&format!("{field} missing")), "{}", f[0].message);
                let label = ord::builtin_register()
                    .lifecycle("standard")
                    .unwrap()
                    .status(status)
                    .unwrap()
                    .label
                    .clone();
                assert!(f[0].message.contains(&label), "{}", f[0].message);
            } else {
                assert!(f.is_empty(), "{status} does not need {field}: {f:?}");
            }
        }
    }
}

#[test]
fn a_blank_sign_off_counts_as_missing_and_all_missing_are_named() {
    let mut v = doc("issued", "0");
    v["control"]["checked"] = "  ".into();
    v["control"].as_object_mut().unwrap().remove("approved");
    let f = findings(&v);
    assert!(f[0].message.contains("checked, approved missing"), "{}", f[0].message);
}

#[test]
fn revision_forms() {
    for ok in ["A", "Z", "AA", "ZZ"] {
        assert!(RevisionForm::Letter.fits(ok), "{ok}");
    }
    for bad in ["", "a", "AAA", "A1", "0", "Ä"] {
        assert!(!RevisionForm::Letter.fits(bad), "{bad}");
    }
    for ok in ["0", "1", "10", "123"] {
        assert!(RevisionForm::Number.fits(ok), "{ok}");
    }
    for bad in ["", "01", "A", "1.0", "-1", " 1"] {
        assert!(!RevisionForm::Number.fits(bad), "{bad}");
    }
}

#[test]
fn a_revision_of_the_wrong_form_is_ord005_both_ways() {
    let f = findings(&doc("issued", "B"));
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD005", "/control/revision"));
    let f = findings(&doc("draft", "1"));
    assert_eq!(f[0].code, "ORD005");
    assert!(f[0].message.contains("letter"));
}

#[test]
fn a_number_without_the_class_prefix_is_ord006() {
    let mut v = doc("issued", "0");
    v["control"]["number"] = "TN-1".into();
    assert_eq!(codes(&v), ["ORD006"]);
    v["control"]["number"] = "RPT1".into();
    assert_eq!(codes(&v), ["ORD006"]);
    v["control"].as_object_mut().unwrap().remove("number");
    assert!(codes(&v).is_empty(), "a number is optional");
}

#[test]
fn dates_are_iso_8601_with_real_days() {
    for ok in ["2026-02-28", "2024-02-29", "2000-02-29", "2026-12-31"] {
        assert!(ord::check::is_iso_date(ok), "{ok}");
    }
    for bad in [
        "2026-2-28",
        "2026-02-29",
        "1900-02-29",
        "2026-13-01",
        "2026-04-31",
        "26-02-28",
        "2026/02/28",
        "2026-02-28T00",
    ] {
        assert!(!ord::check::is_iso_date(bad), "{bad}");
    }
    let mut v = doc("issued", "0");
    v["control"]["date"] = "28/02/2026".into();
    assert_eq!(codes(&v), ["ORD008"]);
}

fn with_history(entries: serde_json::Value) -> serde_json::Value {
    let mut v = doc("issued", "1");
    v["control"]["history"] = entries;
    v
}

#[test]
fn a_consistent_history_passes() {
    let v = with_history(serde_json::json!([
        { "revision": "A", "date": "2026-01-01", "status": "draft", "description": "d" },
        { "revision": "0", "date": "2026-02-01", "status": "Issued", "description": "d" },
        { "revision": "1", "date": "2026-02-28", "status": "final", "description": "d" }
    ]));
    assert!(codes(&v).is_empty(), "{:?}", findings(&v));
}

#[test]
fn history_rules_are_ord007() {
    let repeated = with_history(serde_json::json!([
        { "revision": "1", "date": "2026-01-01", "status": "issued", "description": "d" },
        { "revision": "1", "date": "2026-02-28", "status": "issued", "description": "d" }
    ]));
    assert_eq!(codes(&repeated), ["ORD007"]);
    let backwards = with_history(serde_json::json!([
        { "revision": "0", "date": "2026-03-01", "status": "issued", "description": "d" },
        { "revision": "1", "date": "2026-02-28", "status": "issued", "description": "d" }
    ]));
    let f = findings(&backwards);
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD007", "/control/history/1/date"));
    let not_current = with_history(serde_json::json!([
        { "revision": "0", "date": "2026-02-28", "status": "issued", "description": "d" }
    ]));
    let f = findings(&not_current);
    assert_eq!((f[0].code, f[0].path.as_str()), ("ORD007", "/control/history/0"));
    let bad_status = with_history(serde_json::json!([
        { "revision": "0", "date": "2026-01-01", "status": "sent", "description": "d" },
        { "revision": "1", "date": "2026-02-28", "status": "issued", "description": "d" }
    ]));
    assert_eq!(codes(&bad_status), ["ORD003"]);
}

#[test]
fn a_tools_register_replaces_the_builtin_one() {
    let reg = ord::Register::from_json(
        r#"{
          "classes": [ { "id": "memo", "name": "Memo", "number_prefix": "M", "lifecycle": "simple", "aliases": ["note"] } ],
          "lifecycles": [ { "id": "simple", "statuses": [
              { "id": "open", "label": "Open", "stage": "working", "revision": "number", "furniture": "Open" },
              { "id": "closed", "label": "Closed", "stage": "issued", "revision": "number", "needs": ["approved"] }
          ], "aliases": { "done": "closed" } } ]
        }"#,
    )
    .unwrap();
    let mut v = doc("done", "3");
    v["class"] = "note".into();
    v["control"]["number"] = "M-7".into();
    assert!(findings_with(&v, &reg).is_empty(), "{:?}", findings_with(&v, &reg));
    v["control"].as_object_mut().unwrap().remove("approved");
    assert_eq!(findings_with(&v, &reg)[0].code, "ORD004");
    let report = doc("issued", "0");
    assert_eq!(findings_with(&report, &reg)[0].code, "ORD002", "the built-in classes are gone");
}

#[test]
fn a_register_that_does_not_hold_together_is_refused() {
    let bad = r#"{ "classes": [ { "id": "memo", "name": "Memo", "lifecycle": "none" } ],
                   "lifecycles": [ { "id": "x", "statuses": [], "aliases": { "a": "b" } } ] }"#;
    let err = ord::Register::from_json(bad).unwrap_err();
    let paths: Vec<_> = err.iter().map(|f| (f.code, f.path.as_str())).collect();
    assert_eq!(
        paths,
        [
            ("ORD002", "register:/classes/0/lifecycle"),
            ("ORD002", "register:/lifecycles/0/statuses"),
            ("ORD002", "register:/lifecycles/0/aliases")
        ]
    );
    let typo = ord::Register::from_json(r#"{ "classes": [], "lifecycles": [], "extra": 1 }"#).unwrap_err();
    assert_eq!(typo[0].code, "ORD002");
}

#[test]
fn the_builtin_register_round_trips_through_json() {
    let reg = ord::builtin_register();
    let again = ord::Register::from_json(&serde_json::to_string(&reg).unwrap()).unwrap();
    assert_eq!(reg, again);
}

#[test]
fn each_status_carries_its_furniture() {
    let reg = ord::builtin_register();
    let l = reg.lifecycle("standard").unwrap();
    let f: Vec<_> = l.statuses.iter().map(|s| s.furniture.as_deref()).collect();
    assert_eq!(
        f,
        [Some("Draft"), Some("For checking"), Some("For approval"), None, None, Some("Superseded")]
    );
}

#[test]
fn the_example_raises_no_control_finding() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/screening-report.json");
    let d = ord::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    // The assets are files, not read here, so only the image check (ORD011) may speak.
    let f = ord::check(&d, &Default::default(), &ord::builtin_register());
    assert!(f.iter().all(|f| f.code == "ORD011"), "{f:?}");
}
