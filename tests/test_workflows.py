"""The CI workflows: the required checks always run, filtered jobs never block a merge, caches save from main
only, CodeQL stays off pull requests, and every scheduled workflow reports its failures.

Label: property. No pinned numbers: the tests read .github/workflows.
"""
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
WF = ROOT / ".github" / "workflows"
#: Branch protection requires these check names on main. Renaming one needs a settings change.
REQUIRED = ("test", "release-guard")


def load(name):
    doc = yaml.safe_load((WF / name).read_text(encoding="utf-8"))
    doc["on"] = doc.pop(True, doc.get("on"))  # YAML 1.1 reads the key `on` as true
    return doc


def test_ci_runs_on_pull_requests_and_pushes_to_main_and_tags_only():
    on = load("ci.yml")["on"]
    assert on["push"] == {"branches": ["main"], "tags": ["v*"]}
    assert "paths" not in (on["pull_request"] or {}), "a path filter on ci.yml could skip a required check"


def test_a_new_push_cancels_the_stale_pull_request_run_only():
    c = load("ci.yml")["concurrency"]
    assert "github.event.pull_request.number" in c["group"]
    assert c["cancel-in-progress"] == "${{ github.event_name == 'pull_request' }}"


def test_the_required_jobs_and_the_gate_parts_always_run():
    jobs = load("ci.yml")["jobs"]
    for name in REQUIRED:
        assert jobs[name].get("if", "always()") == "always()", f"{name} must not be conditional"
    for name, job in jobs.items():
        if name.startswith("gate-"):
            assert "if" not in job and "needs" not in job, f"{name} is part of `test`: it must always run"


def test_the_test_check_needs_every_part_of_the_gate_and_fails_unless_all_passed():
    jobs = load("ci.yml")["jobs"]
    parts = sorted(n for n in jobs if n.startswith("gate-"))
    test = jobs["test"]
    assert sorted(test["needs"]) == parts and test["if"] == "always()"
    script = test["steps"][-1]["run"]
    assert f"keys == {str(parts).replace(chr(39), chr(34))}" in script and '.result == "success"' in script


def test_the_parts_run_every_step_of_the_gate_and_every_rust_partition():
    jobs = load("ci.yml")["jobs"]
    steps, partitions = [], []
    for name, job in jobs.items():
        if not name.startswith("gate-"):
            continue
        env = job["steps"][-1]["env"]
        steps += env["GATES_STEPS"].split(",")
        if "GATES_PARTITION" in env:
            partitions = job["strategy"]["matrix"]["partition"]
    gates = (ROOT / "tools" / "gates.sh").read_text(encoding="utf-8")
    all_steps = re.search(r'ALL_STEPS="([^"]+)"', gates).group(1).split(",")
    assert sorted(steps) == sorted(all_steps), "each gate step runs in exactly one part"
    n = len(partitions)
    assert n >= 2 and partitions == [f"{k}/{n}" for k in range(1, n + 1)]


def test_every_other_ci_job_is_path_filtered_or_main_only():
    jobs = load("ci.yml")["jobs"]
    for name, job in jobs.items():
        if name in REQUIRED or name.startswith("gate-") or name == "changes":
            continue
        cond = job.get("if", "")
        assert job.get("needs") == "changes" and "needs.changes.outputs." in cond, \
            f"{name} is not required, so it must run on a pull request only when its inputs change"


def test_the_rust_caches_save_from_main_only():
    for path in WF.glob("*.yml"):
        for job_name, job in load(path.name)["jobs"].items():
            for step in job.get("steps", []):
                if str(step.get("uses", "")).startswith("Swatinem/rust-cache"):
                    assert (step.get("with") or {}).get("save-if") == "${{ github.ref == 'refs/heads/main' }}", \
                        f"{path.name} {job_name}: a pull request must not save a Rust cache"


def test_codeql_is_advanced_setup_on_main_and_weekly_never_on_pull_requests():
    on = load("codeql.yml")["on"]
    assert "pull_request" not in on and on["push"] == {"branches": ["main"]} and "schedule" in on


def test_third_party_actions_outside_the_basics_are_pinned_to_a_commit():
    for path in WF.glob("*.yml"):
        for uses in re.findall(r"uses:\s*(\S+)", path.read_text(encoding="utf-8")):
            if uses.startswith(("dorny/paths-filter", "github/codeql-action")):
                assert re.search(r"@[0-9a-f]{40}$", uses), f"{path.name}: {uses} is not pinned to a commit"


def test_every_scheduled_workflow_reports_a_failed_run():
    watched = set(load("scheduled-failure.yml")["on"]["workflow_run"]["workflows"])
    scheduled = {load(p.name)["name"] for p in WF.glob("*.yml") if "schedule" in (load(p.name)["on"] or {})}
    assert scheduled and scheduled <= watched, f"not reported: {scheduled - watched}"
