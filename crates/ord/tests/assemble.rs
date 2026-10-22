//! Assembly (spec 0001 §9, ORD021). Label: property.

mod common;
use ord::assemble::{Chapter, ChapterStatus, Manifest, assemble};
use serde_json::json;

fn manifest(status: &str, chapters: &[&str], appendices: &[&str]) -> Manifest {
    let mut control = json!({ "revision": "A", "date": "2026-01-01", "status": status });
    if status != "draft" {
        control = json!({ "revision": "0", "date": "2026-01-01", "status": status, "prepared": "p", "checked": "c", "approved": "a" });
    }
    serde_json::from_value(json!({
        "ord": "0.1", "class": "report", "title": "T", "control": control,
        "chapters": chapters, "appendix_chapters": appendices
    }))
    .unwrap()
}

fn chapter(id: &str, status: ChapterStatus) -> Chapter {
    Chapter {
        id: id.into(),
        title: format!("Title {id}"),
        status,
        body: vec![ord::model::Block::Prose(ord::model::Prose { markdown: format!("See [[#{id}]].") })],
    }
}

fn codes(r: Result<ord::Document, Vec<ord::Finding>>) -> Vec<(&'static str, String)> {
    r.unwrap_err().into_iter().map(|f| (f.code, f.path)).collect()
}

#[test]
fn chapters_are_assembled_in_the_manifests_order_by_id() {
    use ChapterStatus::*;
    let chapters = [chapter("c", Agreed), chapter("a", Draft), chapter("b", Outline)];
    let d = assemble(&manifest("draft", &["b", "a"], &["c"]), &chapters, &ord::builtin_register()).unwrap();
    let titles: Vec<_> = d
        .body
        .iter()
        .map(|b| match b {
            ord::model::Block::Section(s) => (s.id.clone().unwrap(), s.title.clone()),
            _ => panic!(),
        })
        .collect();
    assert_eq!(titles, [("b".into(), "Title b".into()), ("a".into(), "Title a".into())]);
    assert_eq!(d.appendices[0].id.as_deref(), Some("c"));
    // The result is an ordinary document: its references resolve and it passes the checks.
    let f = ord::check(&d, &Default::default(), &ord::builtin_register());
    assert!(f.is_empty(), "{f:?}");
}

#[test]
fn a_missing_a_repeated_or_a_doubled_chapter_is_ord021() {
    use ChapterStatus::*;
    let reg = ord::builtin_register();
    let one = [chapter("a", Agreed)];
    assert_eq!(
        codes(assemble(&manifest("draft", &["a", "zz"], &[]), &one, &reg)),
        [("ORD021", "/chapters/1".into())]
    );
    assert_eq!(
        codes(assemble(&manifest("draft", &["a"], &["a"]), &one, &reg)),
        [("ORD021", "/appendix_chapters/0".into())]
    );
    let two = [chapter("a", Agreed), chapter("a", Draft)];
    assert_eq!(codes(assemble(&manifest("draft", &["a"], &[]), &two, &reg))[0].0, "ORD021");
}

#[test]
fn an_issued_document_needs_every_chapter_agreed() {
    use ChapterStatus::*;
    let reg = ord::builtin_register();
    let chapters = [chapter("a", Agreed), chapter("b", Review)];
    for status in ["approved", "issued", "superseded"] {
        let err = assemble(&manifest(status, &["a"], &["b"]), &chapters, &reg).unwrap_err();
        assert_eq!((err[0].code, err[0].path.as_str()), ("ORD021", "/control/status"));
        assert!(err[0].message.contains("\"b\" is \"review\""), "{}", err[0].message);
    }
    assert!(assemble(&manifest("draft", &["a"], &["b"]), &chapters, &reg).is_ok());
    let agreed = [chapter("a", Agreed), chapter("b", Agreed)];
    assert!(assemble(&manifest("issued", &["a"], &["b"]), &agreed, &reg).is_ok());
}

#[test]
fn a_manifest_and_a_chapter_refuse_unknown_fields() {
    let bad = r#"{ "id": "a", "title": "A", "status": "agreed", "body": [], "owner": "x" }"#;
    assert_eq!(ord::parse::parse_as::<Chapter>(bad).unwrap_err()[0].code, "ORD001");
    let bad = r#"{ "id": "a", "title": "A", "status": "done", "body": [] }"#;
    assert_eq!(ord::parse::parse_as::<Chapter>(bad).unwrap_err()[0].path, "/status");
}
