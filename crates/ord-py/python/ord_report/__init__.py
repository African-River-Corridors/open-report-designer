"""Open Report Designer: one typed report document to PDF, HTML and Word (spec 0001 §11.3).

    import ord_report
    pdf = ord_report.render_file("report.json", "pdf")
    html = ord_report.render(document, "html", assets={"plan": png_bytes})

A document is a dict or a JSON string. A document with errors raises DocumentError, whose `findings` are
dicts with `code`, `severity`, `path` and `message`.
"""
from __future__ import annotations

import json
import warnings as _warnings
from pathlib import Path
from typing import Any, Mapping, Sequence

from . import _ord

__version__: str = _ord.__version__

__all__ = ["DocumentError", "FindingWarning", "render", "render_file", "check", "assemble", "schema",
           "builtin_register", "__version__"]

FORMATS = ("pdf", "html", "docx")


class DocumentError(ValueError):
    """The document has errors. `findings` lists them all."""

    def __init__(self, findings: list[dict]):
        self.findings = findings
        lines = [f"{f['severity']} {f['code']} at {f['path'] or '/'}: {f['message']}" for f in findings]
        super().__init__("\n".join(lines) or "the document has errors")


class FindingWarning(UserWarning):
    """A warning finding raised while rendering."""


def _json(value: str | Mapping[str, Any] | None) -> str | None:
    if value is None or isinstance(value, str):
        return value
    return json.dumps(value)


def _call(fn, *args):
    try:
        return fn(*args)
    except _ord.FindingsError as e:
        raise DocumentError(json.loads(e.args[0])) from None


def render(document: str | Mapping[str, Any], format: str, assets: Mapping[str, bytes] | None = None,
           register: str | Mapping[str, Any] | None = None) -> bytes:
    """Renders a document as "pdf", "html" or "docx". `assets` gives the bytes of `path` assets by id.
    Warnings are issued as FindingWarning."""
    out, warns = _call(_ord.render, _json(document), format, dict(assets or {}), _json(register))
    for w in json.loads(warns):
        _warnings.warn(f"{w['code']} at {w['path'] or '/'}: {w['message']}", FindingWarning, stacklevel=2)
    return out


def _file_assets(doc: Mapping[str, Any], folder: Path) -> dict[str, bytes]:
    return {aid: (folder / a["path"]).read_bytes() for aid, a in doc.get("assets", {}).items() if "path" in a}


def render_file(path: str | Path, format: str, out: str | Path | None = None,
                register: str | Mapping[str, Any] | None = None) -> bytes:
    """Renders a document file. `path` assets are read relative to the file. Writes `out` if given."""
    path = Path(path)
    doc = json.loads(path.read_text(encoding="utf-8"))
    data = render(doc, format, _file_assets(doc, path.parent), register)
    if out is not None:
        Path(out).write_bytes(data)
    return data


def check(document: str | Mapping[str, Any], assets: Mapping[str, bytes] | None = None,
          register: str | Mapping[str, Any] | None = None) -> list[dict]:
    """Every finding about a document (errors and warnings), as dicts."""
    return json.loads(_call(_ord.check, _json(document), dict(assets or {}), _json(register)))


def assemble(manifest: str | Mapping[str, Any], chapters: Sequence[str | Mapping[str, Any]],
             register: str | Mapping[str, Any] | None = None) -> dict:
    """One document from a manifest and its chapters (found by id)."""
    return json.loads(_call(_ord.assemble, _json(manifest), [_json(c) for c in chapters], _json(register)))


def schema() -> str:
    """The JSON Schema of a document."""
    return _ord.schema()


def builtin_register() -> dict:
    """The built-in register of document classes."""
    return json.loads(_ord.builtin_register())
