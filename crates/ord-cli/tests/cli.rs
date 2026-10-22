//! The `ord` program, run as a user runs it (spec 0001 §11.2). Label: property.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ord(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ord")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ord-cli-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn render_writes_the_file_named_after_the_document() {
    let out = tmp("render");
    let doc = repo().join("examples/screening-report.json");
    let o = ord(&["render", s(&doc), "--format", "html", "-o", s(&out)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let html = std::fs::read_to_string(out.join("screening-report.html")).unwrap();
    assert!(html.contains("Heron River navigation screening"));
}

#[test]
fn render_all_writes_pdf_html_and_word() {
    let out = tmp("all");
    let doc = repo().join("examples/screening-report.json");
    let o = ord(&["render", s(&doc), "--format", "all", "-o", s(&out)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    for (ext, magic) in [("pdf", &b"%PDF-"[..]), ("html", b"<!doctype html>"), ("docx", b"PK\x03\x04")] {
        let bytes = std::fs::read(out.join(format!("screening-report.{ext}"))).unwrap();
        assert!(bytes.starts_with(magic), "{ext}");
    }
    // The output equals the library's: the program adds nothing of its own.
    let (d, a) = {
        let doc =
            ord::parse(&std::fs::read_to_string(repo().join("examples/screening-report.json")).unwrap())
                .unwrap();
        let dir = repo().join("examples");
        let sup = doc
            .assets
            .iter()
            .map(|(k, v)| (k.clone(), std::fs::read(dir.join(v.path.as_ref().unwrap())).unwrap()))
            .collect();
        let (a, _) = ord::assets::collect(&doc, &sup);
        (doc, a)
    };
    let lib = ord::render(&d, &a, &ord::builtin_register(), ord::Format::Docx).unwrap().bytes;
    assert_eq!(std::fs::read(out.join("screening-report.docx")).unwrap(), lib);
}

#[test]
fn check_is_clean_on_the_example_and_prints_json_on_request() {
    let doc = repo().join("examples/screening-report.json");
    let o = ord(&["check", s(&doc)]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("no findings"));
    let o = ord(&["check", s(&doc), "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v, serde_json::json!([]));
}

#[test]
fn a_document_with_errors_exits_1_and_names_the_file_and_code() {
    let dir = tmp("bad");
    let p = dir.join("bad.json");
    std::fs::write(
        &p,
        r#"{ "ord": "0.1", "class": "memo", "title": "T",
        "control": { "revision": "A", "date": "2026-01-01", "status": "draft" }, "body": [] }"#,
    )
    .unwrap();
    let o = ord(&["check", s(&p)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("ORD002"));
    let o = ord(&["render", s(&p), "-o", s(&dir)]);
    assert_eq!(o.status.code(), Some(1));
    std::fs::write(&p, "{ nope").unwrap();
    let o = ord(&["check", s(&p)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("bad.json:"));
}

#[test]
fn usage_and_input_output_errors_exit_2() {
    assert_eq!(ord(&["check", "/no/such/file.json"]).status.code(), Some(2));
    let doc = repo().join("examples/screening-report.json");
    assert_eq!(ord(&["render", s(&doc), "--format", "odt"]).status.code(), Some(2));
    assert_eq!(ord(&["frobnicate"]).status.code(), Some(2));
}

#[test]
fn schema_and_register_print_json() {
    let o = ord(&["schema"]);
    assert_eq!(String::from_utf8(o.stdout).unwrap(), ord::schema());
    let o = ord(&["register"]);
    let r = ord::Register::from_json(std::str::from_utf8(&o.stdout).unwrap()).unwrap();
    assert_eq!(r, ord::builtin_register());
}

#[test]
fn a_tools_register_replaces_the_builtin_one() {
    let dir = tmp("register");
    let reg = dir.join("reg.json");
    std::fs::write(&reg, r#"{ "classes": [ { "id": "memo", "name": "Memo", "lifecycle": "l" } ],
        "lifecycles": [ { "id": "l", "statuses": [ { "id": "open", "label": "Open", "stage": "working", "revision": "letter" } ] } ] }"#).unwrap();
    let doc = dir.join("memo.json");
    std::fs::write(&doc, r#"{ "ord": "0.1", "class": "memo", "title": "T",
        "control": { "revision": "A", "date": "2026-01-01", "status": "open" }, "body": [ { "type": "prose", "markdown": "x" } ] }"#).unwrap();
    assert_eq!(ord(&["check", s(&doc)]).status.code(), Some(1));
    assert_eq!(ord(&["check", s(&doc), "--register", s(&reg)]).status.code(), Some(0));
}

#[test]
fn assemble_finds_chapters_by_id_and_rebases_asset_paths() {
    let out = tmp("assemble");
    let ex = repo().join("examples/chaptered");
    let doc = out.join("sub/note.json");
    let o = ord(&[
        "assemble",
        s(&ex.join("manifest.json")),
        "--chapters",
        s(&ex.join("chapters")),
        "-o",
        s(&doc),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let d = ord::parse(&std::fs::read_to_string(&doc).unwrap()).unwrap();
    assert_eq!(d.body.len(), 2);
    assert_eq!(d.appendices[0].title, "Bend list");
    // The asset path now points from the new document's folder to the same file.
    let rel = d.assets["key"].path.clone().unwrap();
    assert!(std::fs::read(doc.parent().unwrap().join(&rel)).is_ok(), "{rel}");
    let o = ord(&["render", s(&doc), "--format", "html", "-o", s(&out)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let html = std::fs::read_to_string(out.join("note.html")).unwrap();
    assert!(html.contains("Appendix A  Bend list") && html.contains("For approval"));
}

#[test]
fn assemble_refuses_an_issued_document_with_a_chapter_not_agreed() {
    let dir = tmp("issued");
    let ex = repo().join("examples/chaptered");
    let mut m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ex.join("manifest.json")).unwrap()).unwrap();
    m["control"] = serde_json::json!({ "number": "TN-0007", "revision": "0", "date": "2026-10-21", "status": "issued",
        "prepared": "p", "checked": "c", "approved": "a" });
    let mp = dir.join("manifest.json");
    std::fs::write(&mp, m.to_string()).unwrap();
    let o = ord(&["assemble", s(&mp), "--chapters", s(&ex.join("chapters")), "-o", s(&dir.join("x.json"))]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("ORD021"));
}
