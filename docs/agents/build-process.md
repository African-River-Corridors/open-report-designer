# Build process

How ticketed work is built and merged. It follows the sister repositories' process from day one, in
**single-builder mode**: one builder works the tickets one at a time.

## The loop

A spec is a parent issue; its tickets are sub-issues. Native GitHub "blocked by" links are the edges of the
task graph. The builder takes the next ticket whose blockers are all closed, and no other. For each ticket:

1. **Read.** Read the ticket and its comments (`gh issue view <n> --comments`). Later comments may revise the
   acceptance criteria.
2. **Build.** Work in the ticket's own git worktree, on a branch `ticket/<n>-<slug>` cut from an up-to-date
   `origin/main`. Test first. One pull request to `main` closes one ticket. Every acceptance criterion ticked
   in the pull request body links to its evidence: a test in the diff, a command and its output in the body,
   or a CI job.
3. **Local checks, targeted.** CI is the full gate, so do not run all of `tools/gates.sh` before every push.
   Run what the change can break:
   - `cargo fmt --all --check`;
   - clippy with warnings as errors on the crates you touched (and `GATES_STEPS=wasm bash tools/gates.sh`
     if you touched the engine);
   - the tests of the crates or Python modules you touched;
   - `python3 tools/release_guard.py` and `--history`;
   - `python3 tools/changelog.py check`;
   - the embargo check of the pull request body (`issue-tracker.md`).

   Run the whole `bash tools/gates.sh` only for a change across many crates, or to reproduce a red CI run.
4. **Auto-merge at once; never wait on CI.** As soon as the pull request is open:
   `gh pr merge <pr> --squash --auto --delete-branch`. Then start the next ticket's reading and tests. Branch
   protection requires `test` and `release-guard`, so auto-merge cannot land red code. `strict` is off, so a
   pull request does not need a rebase each time `main` moves. If a pull request goes red, fixing it comes
   before anything else.
5. **After the merge.** Check that the pull request merged, the issue closed, and `origin/main` holds the
   squash commit. Write the **build notes**: one closing comment on the ticket (what was built, the commands
   and their result, anything not done, the pull request link), checked with the embargo one-liner. Remove the
   worktree.

## One build folder per builder

Each builder sets `CARGO_TARGET_DIR=$HOME/Dev/_cargo-target/ord` for every worktree, so a fresh worktree
reuses the compiled dependencies. Build one worktree at a time: two worktrees in one build folder can swap
each other's test binaries. When you switch worktrees, `cargo clean -p ord` (and the other workspace crates)
first.

## Which CI jobs run where

| Job | On a pull request | On main | Other |
|---|---|---|---|
| CI `test` (needs `gate-lint`, `gate-rust` ×2, `gate-python`), `release-guard` | always | always | by hand |
| Other CI jobs | only when their inputs change (`changes` job, path filter) | always | by hand |
| Nightly (every slow test) | its own files | – | daily, by hand |
| CodeQL (`codeql.yml`, advanced setup) | never | always | weekly, by hand |

The required `test` check is a small job that needs the parts of the gate, which run in parallel:
`gate-lint` (fmt, clippy, the wasm32 build), `gate-rust` (the Rust tests in nextest hash partitions, the
doctests in the first) and `gate-python` (pytest, with the bindings built first). It fails unless every part
passed. The Rust caches save from `main` only. A failed scheduled run opens an issue (`scheduled-failure.yml`).
`tests/test_workflows.py` checks all of this.

Slow tests move to the nightly profile only by measured time, each with a reason in `slow-tests.md`.

## Changelog fragments

**No pull request edits `CHANGELOG.md`.** Add a fragment, `changelog.d/<ticket>.<kind>.md` (kinds: `added`,
`changed`, `deprecated`, `removed`, `fixed`, `security`), with one Markdown entry and no heading. Only the
release pull request touches `CHANGELOG.md`: it runs `python3 tools/changelog.py release <version>`.

## Mutation testing is a later hardening pass

Builders do not run mutation tests during the build. Mutation testing runs as its own pass once a spec's
feature tickets are merged and accepted, one run at a time, scoped to modules that have stopped changing.
Each surviving mutant that can change a public output becomes a fix-forward ticket.

## Release

A version is a tag `vX.Y.Z` on `main`, made after the release pull request (version bump and
`tools/changelog.py release`) merges, with a GitHub release. The release workflow attaches the example report
in all three formats. A tag is cut only when every acceptance criterion of the release's spec is met.

## Stuck tickets

If one problem is still open after two honest attempts, label the ticket `ready-for-human`, comment why in
plain English (checked with the embargo one-liner), and move to the next ticket whose blockers are closed.

## Rules every agent follows

- One pull request per ticket, straight to `main`. Never push to `main` directly.
- Write "closes #N" only for the ticket the pull request really closes.
- This repo is public. Check every issue, pull request body and comment with the embargo one-liner, and run
  the release guard with `--history` before every push. New files need `git add -N` before the guard sees
  them. No people's names in public text.
- Several worktrees share one clone. Stage files by name, never with `git add -A`.
- Never stop a process by name; stop only a process you started, by its process id.
- Long builds and test runs go to the background with their output in a file.

## Fixed-number tests: labels

A test that pins a number carries a `//! Label:` line in its file header (`# Label:` in Python):

- `Label: sourced - <citation>`: the number comes from a standard or a hand calculation;
- `Label: snapshot`: the number is what the code printed; it shows the code still agrees, not that it is right;
- `Label: property`: no fixed number.
