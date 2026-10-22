#!/usr/bin/env bash
# The gate (spec 0001): one fail-closed script, run by the CI parts of the required `test` check and by hand.
#
#   bash tools/gates.sh
#
# Fail-closed: a missing tool fails before any step runs, any failing step stops the script, and the
# script fails unless every step it was asked for ran. There is no skip switch.
#
# Steps:
#   fmt      cargo fmt --check
#   clippy   clippy on the native crates, warnings as errors
#   wasm     the engine builds and lints for wasm32-unknown-unknown (it must run in a browser)
#   rust     cargo nextest (profile `pr`, .config/nextest.toml) and `cargo test --doc`. The slowest tests
#            run nightly instead (docs/agents/slow-tests.md); GATES_NEXTEST_PROFILE=nightly runs them all.
#   pytest   the Python tests: the tools, the workflows, and the Python bindings when they are built
#
# CI runs the gate in parallel parts (the `gate-*` jobs in .github/workflows/ci.yml). Two settings pick a
# part; with neither set the script runs every step:
#   GATES_STEPS      the steps to run, by name, separated by commas. An unknown name fails.
#   GATES_PARTITION  k/N: the rust step runs only the k-th of N hash partitions. Doctests run in partition 1.
#
# ORD_PYTHON picks the Python for the pytest step (default python3).
set -euo pipefail

fail() { echo "gates: FAIL: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "required tool '$1' is not on PATH"; }

need dirname
cd "$(dirname "$0")/.."
PY="${ORD_PYTHON:-python3}"
ALL_STEPS="fmt,clippy,wasm,rust,pytest"
STEPS="${GATES_STEPS:-$ALL_STEPS}"
PARTITION="${GATES_PARTITION:-}"
# Crates that the native steps leave out: each has its own step or job.
NATIVE_EXCLUDE=()
[ -d crates/ord-wasm ] && NATIVE_EXCLUDE+=(--exclude ord-wasm)
[ -d crates/ord-py ] && NATIVE_EXCLUDE+=(--exclude ord-py)

EXPECTED_STEPS=0
steps_run=0
for s in ${STEPS//,/ }; do
  case ",$ALL_STEPS," in
    *",$s,"*) EXPECTED_STEPS=$((EXPECTED_STEPS + 1)) ;;
    *) fail "unknown step '$s' in GATES_STEPS (known: $ALL_STEPS)" ;;
  esac
done
[ "$EXPECTED_STEPS" -gt 0 ] || fail "GATES_STEPS names no step"
wants() { case ",$STEPS," in *",$1,"*) return 0 ;; *) return 1 ;; esac; }
if [ -n "$PARTITION" ]; then
  [[ "$PARTITION" =~ ^([1-9][0-9]*)/([1-9][0-9]*)$ ]] && [ "${BASH_REMATCH[1]}" -le "${BASH_REMATCH[2]}" ] \
    || fail "GATES_PARTITION must be k/N with 1 <= k <= N, not '$PARTITION'"
fi

need cargo
need rustc
if wants fmt; then cargo fmt --version >/dev/null 2>&1 || fail "rustfmt is not installed (rustup component add rustfmt)"; fi
if wants clippy || wants wasm; then cargo clippy --version >/dev/null 2>&1 || fail "clippy is not installed (rustup component add clippy)"; fi
if wants wasm; then rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown \
  || fail "the wasm32-unknown-unknown target is not installed (rustup target add wasm32-unknown-unknown)"; fi
if wants rust; then cargo nextest --version >/dev/null 2>&1 || fail "cargo-nextest is not installed (cargo install cargo-nextest --locked)"; fi
if wants pytest; then need "$PY"; "$PY" -c "import pytest, yaml" 2>/dev/null || fail "pytest and pyyaml are not importable by $PY (pip install -r requirements-dev.txt)"; fi

# "$@" runs inside `||`, so bash ignores `set -e` in it: a step that is a function returns non-zero itself.
step() {
  local name="$1"; shift
  local start=$SECONDS
  echo "::group::gates: $name"
  "$@" || { echo "::endgroup::"; fail "$name"; }
  echo "::endgroup::"
  steps_run=$((steps_run + 1))
  echo "gates: ok: $name ($((SECONDS - start)) s)"
}

rust_tests() {
  local part=()
  [ -n "$PARTITION" ] && part=(--partition "hash:$PARTITION")
  cargo nextest run --locked --workspace ${NATIVE_EXCLUDE[@]+"${NATIVE_EXCLUDE[@]}"} --all-features \
    --profile "${GATES_NEXTEST_PROFILE:-pr}" --no-tests=warn ${part[@]+"${part[@]}"} || return 1
  # nextest does not run doctests. They run once: with no partition, or in partition 1.
  if [ -z "$PARTITION" ] || [ "${PARTITION%%/*}" = 1 ]; then
    cargo test --locked --doc --workspace ${NATIVE_EXCLUDE[@]+"${NATIVE_EXCLUDE[@]}"} --all-features || return 1
  fi
}

wasm_build() {
  local crates=(-p ord)
  [ -d crates/ord-wasm ] && crates+=(-p ord-wasm)
  cargo clippy --locked --target wasm32-unknown-unknown "${crates[@]}" -- -D warnings || return 1
}

python_tests() {
  # The bindings, when the crate exists, are built into the running Python first (maturin develop).
  if [ -d crates/ord-py ]; then
    "$PY" -c "import maturin" 2>/dev/null || need maturin
    (cd crates/ord-py && "$PY" -m maturin develop --locked --release -q) || return 1
  fi
  "$PY" -m pytest -q || return 1
}

if wants fmt; then step "cargo fmt --check" cargo fmt --all --check; fi
if wants clippy; then
  step "clippy -D warnings" cargo clippy --locked --workspace ${NATIVE_EXCLUDE[@]+"${NATIVE_EXCLUDE[@]}"} --all-targets --all-features -- -D warnings
fi
if wants wasm; then step "wasm32 build and clippy" wasm_build; fi
if wants rust; then step "rust tests${PARTITION:+, partition $PARTITION}" rust_tests; fi
if wants pytest; then step "pytest" python_tests; fi

[ "$steps_run" -eq "$EXPECTED_STEPS" ] || fail "only $steps_run of $EXPECTED_STEPS steps ran"
echo "gates: all $EXPECTED_STEPS steps passed in $SECONDS s ($STEPS${PARTITION:+, partition $PARTITION})"
