"""The Python bindings, ord_report (spec 0001 §11.3, §12 "Bindings").

Label: property. No pinned numbers: outputs are compared with the native `ord` program's.
"""
import json
import os
import subprocess
import warnings
from pathlib import Path

import pytest

ord_report = pytest.importorskip("ord_report")

ROOT = Path(__file__).resolve().parents[3]
EXAMPLE = ROOT / "examples" / "screening-report.json"


def native(fmt: str, tmp_path: Path) -> bytes:
    """The same document rendered by the native `ord` program (ORD_CLI, set by tools/gates.sh)."""
    cli = os.environ.get("ORD_CLI")
    if not cli:
        pytest.skip("ORD_CLI is not set: run through tools/gates.sh")
    subprocess.run([cli, "render", str(EXAMPLE), "--format", fmt, "-o", str(tmp_path)], check=True,
                   capture_output=True)
    return (tmp_path / f"screening-report.{fmt}").read_bytes()


@pytest.mark.parametrize("fmt", ["pdf", "html", "docx"])
def test_the_example_renders_and_equals_the_native_output(fmt, tmp_path):
    out = ord_report.render_file(EXAMPLE, fmt)
    assert out == native(fmt, tmp_path)
    magic = {"pdf": b"%PDF-", "html": b"<!doctype html>", "docx": b"PK\x03\x04"}[fmt]
    assert out.startswith(magic)


def test_render_file_writes_out(tmp_path):
    target = tmp_path / "r.html"
    data = ord_report.render_file(EXAMPLE, "html", out=target)
    assert target.read_bytes() == data


def test_a_dict_document_with_supplied_assets_renders():
    doc = json.loads(EXAMPLE.read_text(encoding="utf-8"))
    assets = {k: (EXAMPLE.parent / v["path"]).read_bytes() for k, v in doc["assets"].items()}
    assert ord_report.render(doc, "html", assets=assets) == ord_report.render_file(EXAMPLE, "html")


def test_a_document_with_errors_raises_document_error_with_its_findings():
    doc = json.loads(EXAMPLE.read_text(encoding="utf-8"))
    doc["class"] = "memo"
    with pytest.raises(ord_report.DocumentError) as e:
        ord_report.render(doc, "html")
    codes = [f["code"] for f in e.value.findings]
    assert "ORD002" in codes and "ORD011" in codes  # unknown class; path assets not supplied
    assert "ORD002" in str(e.value)


def test_bad_json_and_bad_format():
    with pytest.raises(ord_report.DocumentError) as e:
        ord_report.render("{ nope", "html")
    assert e.value.findings[0]["code"] == "ORD001"
    with pytest.raises(ValueError):
        ord_report.render(json.loads(EXAMPLE.read_text(encoding="utf-8")), "odt")


def test_warnings_are_issued_as_finding_warnings():
    doc = {"ord": "0.1", "class": "report", "title": "T",
           "control": {"revision": "A", "date": "2026-01-01", "status": "draft"},
           "body": [{"type": "section", "title": "Empty", "body": []}]}
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        ord_report.render(doc, "html")
    assert any("ORD018" in str(x.message) and x.category is ord_report.FindingWarning for x in w)


def test_check_returns_every_finding():
    doc = json.loads(EXAMPLE.read_text(encoding="utf-8"))
    assert ord_report.check(doc, assets={
        k: (EXAMPLE.parent / v["path"]).read_bytes() for k, v in doc["assets"].items()
    }) == []
    f = ord_report.check({"ord": "0.2"})
    assert f[0]["code"] == "ORD001"


def test_assemble_schema_register_and_version():
    ex = ROOT / "examples" / "chaptered"
    manifest = json.loads((ex / "manifest.json").read_text(encoding="utf-8"))
    chapters = [json.loads(p.read_text(encoding="utf-8")) for p in sorted((ex / "chapters").rglob("*.json"))]
    doc = ord_report.assemble(manifest, chapters)
    assert [s["title"] for s in doc["body"]] == ["Purpose", "Findings"]
    assert json.loads(ord_report.schema())["title"] == "Open Report Designer document"
    assert ord_report.builtin_register()["classes"][0]["id"] == "report"
    assert ord_report.__version__
    custom = {"classes": [{"id": "memo", "name": "Memo", "lifecycle": "l"}],
              "lifecycles": [{"id": "l", "statuses": [
                  {"id": "open", "label": "Open", "stage": "working", "revision": "letter"}]}]}
    memo = {"ord": "0.1", "class": "memo", "title": "T",
            "control": {"revision": "A", "date": "2026-01-01", "status": "open"},
            "body": [{"type": "prose", "markdown": "x"}]}
    assert ord_report.check(memo, register=custom) == []
