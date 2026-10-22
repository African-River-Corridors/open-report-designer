"""The release guard can fail: it finds a planted term, honours the allowlist, and scans history.

Label: property. The planted terms are invented words hashed here, so the real list stays unpublished.
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
import embargo  # noqa: E402
import release_guard as rg  # noqa: E402

PLANTED = frozenset({embargo.term_hash("zorblax"), embargo.term_hash("quill fen")})
ZORBLAX = embargo.term_id("zorblax")
QUILL = embargo.term_id("quill fen")


def _repo(tmp_path, files, allow=""):
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    for name, text in files.items():
        (tmp_path / name).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / name).write_text(text)
    if allow:
        (tmp_path / rg.ALLOW_FILE).write_text(allow)
    subprocess.run(["git", "-C", str(tmp_path), "add", "."], check=True)
    return tmp_path


def _commit(root):
    subprocess.run(["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t@example.org",
                    "commit", "-q", "-m", "plain message"], check=True)


def test_finds_a_planted_term_whole_word_and_case_insensitive(tmp_path):
    root = _repo(tmp_path, {"a.md": "ok\nthe Zorblax report\nzorblaxes are fine\n"})
    hits, _ = rg.scan(root, PLANTED)
    assert hits == [f"a.md:2:{ZORBLAX}"]


def test_a_two_word_term_wrapped_over_two_lines_is_found(tmp_path):
    root = _repo(tmp_path, {"a.md": "the quill\nfen tool\n"})
    hits, _ = rg.scan(root, PLANTED)
    assert hits == [f"a.md:1:{QUILL}"]


def test_the_allowlist_excuses_only_its_glob(tmp_path):
    root = _repo(tmp_path, {"docs/a.md": "zorblax\n", "b.md": "zorblax\n"},
                 allow=f"docs/*  {ZORBLAX}  # reason\n")
    # The allowlist resolves ids against the real list; plant the term there for this test.
    old = embargo.TERM_HASHES
    embargo.TERM_HASHES = old | PLANTED
    try:
        hits, ok = rg.scan(root, PLANTED)
    finally:
        embargo.TERM_HASHES = old
    assert hits == [f"b.md:1:{ZORBLAX}"] and ok == [f"docs/a.md:1:{ZORBLAX}"]


def test_history_mode_finds_a_term_that_was_deleted_from_the_tree(tmp_path):
    root = _repo(tmp_path, {"a.md": "zorblax\n"})
    _commit(root)
    (root / "a.md").write_text("clean\n")
    subprocess.run(["git", "-C", str(root), "add", "a.md"], check=True)
    _commit(root)
    assert rg.scan(root, PLANTED)[0] == []
    hits, _ = rg.scan_history(root, PLANTED)
    assert len(hits) == 1 and hits[0].endswith(f":a.md:1:{ZORBLAX}")


def test_the_repo_itself_is_clean():
    assert rg.main([]) == 0
