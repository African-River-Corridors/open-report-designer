//! Reading documents, and the published schema (spec 0001 §3, §11.1, §12 "Schema").
//! Label: property. No pinned numbers.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example() -> String {
    std::fs::read_to_string(repo().join("examples/screening-report.json")).unwrap()
}

fn minimal() -> serde_json::Value {
    serde_json::json!({
        "ord": "0.1", "class": "report", "title": "T",
        "control": { "revision": "A", "date": "2026-01-01", "status": "draft" },
        "body": [ { "type": "prose", "markdown": "Hello." } ]
    })
}

fn parse_value(v: &serde_json::Value) -> Result<ord::Document, Vec<ord::Finding>> {
    ord::parse(&v.to_string())
}

#[test]
fn the_example_parses_and_round_trips() {
    let doc = ord::parse(&example()).unwrap();
    assert_eq!(doc.class, "report");
    assert_eq!(doc.appendices.len(), 2);
    let again = ord::parse(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert_eq!(doc, again);
}

#[test]
fn bad_json_is_ord001() {
    let err = ord::parse("{ not json").unwrap_err();
    assert_eq!(err.len(), 1);
    assert_eq!(err[0].code, "ORD001");
    assert!(err[0].is_error());
}

#[test]
fn an_unknown_top_level_field_is_refused_with_its_path() {
    let mut v = minimal();
    v["titel"] = "typo".into();
    let err = parse_value(&v).unwrap_err();
    assert_eq!(err[0].code, "ORD001");
    assert!(err[0].message.contains("titel"), "{}", err[0].message);
}

#[test]
fn an_unknown_field_inside_a_block_is_refused_with_a_pointer_to_the_block() {
    let mut v = minimal();
    v["body"][0]["colour"] = "red".into();
    let err = parse_value(&v).unwrap_err();
    assert_eq!(err[0].code, "ORD001");
    assert!(err[0].message.contains("colour"), "{}", err[0].message);
    assert!(err[0].path.starts_with("/body/0"), "{}", err[0].path);
}

#[test]
fn an_unknown_field_in_a_nested_table_column_names_the_column() {
    let mut v = minimal();
    v["body"] = serde_json::json!([{ "type": "section", "title": "S", "body": [
        { "type": "table", "caption": "C", "columns": [ { "title": "A", "wdth": 3 } ], "rows": [] }
    ]}]);
    let err = parse_value(&v).unwrap_err();
    assert!(err[0].path.starts_with("/body/0/body/0/columns/0"), "{}", err[0].path);
}

#[test]
fn a_wrong_type_is_refused_with_its_path() {
    let mut v = minimal();
    v["control"]["date"] = 20260101.into();
    let err = parse_value(&v).unwrap_err();
    assert_eq!(err[0].path, "/control/date");
}

#[test]
fn an_unknown_block_type_is_refused() {
    let mut v = minimal();
    v["body"][0]["type"] = "video".into();
    let err = parse_value(&v).unwrap_err();
    assert!(err[0].message.contains("video"), "{}", err[0].message);
}

#[test]
fn another_format_version_is_refused_at_ord() {
    let mut v = minimal();
    v["ord"] = "0.2".into();
    let err = parse_value(&v).unwrap_err();
    assert_eq!((err[0].code, err[0].path.as_str()), ("ORD001", "/ord"));
}

#[test]
fn a_missing_required_field_is_refused() {
    let mut v = minimal();
    v["control"].as_object_mut().unwrap().remove("revision");
    let err = parse_value(&v).unwrap_err();
    assert!(err[0].message.contains("revision"), "{}", err[0].message);
}

#[test]
fn inline_assets_decode_and_path_assets_need_supplied_bytes() {
    let mut v = minimal();
    v["assets"] = serde_json::json!({
        "a": { "media_type": "image/png", "base64": "iVBORw0KGgo=" },
        "b": { "media_type": "image/svg+xml", "path": "b.svg" },
        "c": { "media_type": "image/png", "base64": "@@@" }
    });
    let doc = parse_value(&v).unwrap();
    let (assets, findings) = ord::assets::collect(&doc, &Default::default());
    assert_eq!(assets["a"].bytes, b"\x89PNG\r\n\x1a\n");
    assert!(!assets.contains_key("b") && !assets.contains_key("c"));
    let codes: Vec<_> = findings.iter().map(|f| (f.code, f.path.as_str())).collect();
    assert_eq!(codes, [("ORD011", "/assets/b"), ("ORD011", "/assets/c")]);

    let supplied = [("b".to_string(), b"<svg/>".to_vec())].into_iter().collect();
    let (assets, findings) = ord::assets::collect(&doc, &supplied);
    assert_eq!(assets["b"].bytes, b"<svg/>");
    assert_eq!(findings.len(), 1);
}

#[test]
fn the_committed_schema_is_the_generated_one() {
    let path = repo().join("schema/ord-document.schema.json");
    let generated = ord::schema();
    if std::env::var_os("ORD_BLESS").is_some() {
        std::fs::write(&path, &generated).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "schema/ord-document.schema.json is stale: run `ORD_BLESS=1 cargo test -p ord --test document`"
    );
}

#[test]
fn the_example_validates_against_the_schema_with_an_independent_validator() {
    let schema: serde_json::Value = serde_json::from_str(&ord::schema()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&example()).unwrap();
    let errors: Vec<String> = validator.iter_errors(&doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn the_schema_refuses_what_the_parser_refuses() {
    let schema: serde_json::Value = serde_json::from_str(&ord::schema()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut v = minimal();
    assert!(validator.is_valid(&v));
    v["body"][0]["colour"] = "red".into();
    assert!(!validator.is_valid(&v), "the schema must refuse an unknown block field");
    let mut v = minimal();
    v["control"]["sensitivity"] = "secret".into();
    assert!(!validator.is_valid(&v));
    assert!(parse_value(&v).is_err());
}

#[test]
fn no_finding_message_leaks_the_internal_path_mark() {
    let mut v = minimal();
    v["body"] = serde_json::json!([{ "type": "section", "title": "S", "body": [{ "type": "figure", "caption": 3 }] }]);
    let err = parse_value(&v).unwrap_err();
    assert_eq!(err[0].path, "/body/0/body/0/caption");
    assert!(!err[0].message.contains('\u{1}') && !err[0].message.contains(" at line "), "{}", err[0].message);
}
