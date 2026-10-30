"""The Python package's metadata: the licence copies in crates/ord-py equal the originals, and the
distribution and import names stay what PyPI and users know.

Label: property. No pinned numbers: the tests read the files.
"""
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "crates" / "ord-py"


def test_the_licence_copies_in_the_python_package_equal_the_originals():
    pairs = [(ROOT / "LICENSE", PY / "LICENSE"), (ROOT / "NOTICE", PY / "NOTICE")]
    fonts = sorted((ROOT / "crates" / "ord" / "fonts").glob("OFL-*.txt"))
    assert fonts, "no font licences found"
    pairs += [(f, PY / "licenses" / f.name) for f in fonts]
    for original, copy in pairs:
        assert copy.read_bytes() == original.read_bytes(), f"{copy.relative_to(ROOT)} differs: copy it again"
    assert sorted(p.name for p in (PY / "licenses").iterdir()) == [f.name for f in fonts]


def test_the_distribution_is_open_report_designer_and_the_import_is_ord_report():
    meta = tomllib.loads((PY / "pyproject.toml").read_text(encoding="utf-8"))
    assert meta["project"]["name"] == "open-report-designer"
    assert meta["tool"]["maturin"]["module-name"].split(".")[0] == "ord_report"
    assert meta["project"]["license-files"] == ["LICENSE", "NOTICE", "licenses/OFL-*.txt"]
