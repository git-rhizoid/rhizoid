# AGENTS.md - how to work in rhizoid

Fork, own, and sync git dependencies instead of relying on a package manager - see the README for
what that means and [`git-rhizoid/git-rhizoid.github.io`](https://github.com/git-rhizoid/git-rhizoid.github.io)
for the design log. Scaffolded from [grenudi/rustnix](https://github.com/grenudi/rustnix): a Nix +
direnv devenv (isolated VSCodium, pinned toolchain - see `README.md`) plus CI, release automation,
and a test convention, all wired up from the first commit.

## Setup (once per session)
```
./init_devenv.sh   # trusts .envrc and loads the Nix devshell; see README.md
cargo test --workspace --all-features
```

## Conventions
1. **Conventional Commits on the PR title**, not necessarily every commit inside it - `pr-title.yml`
   lints the title, and `release-plz` reads the squash-merged result to decide the version bump and
   changelog entry. See `CONTRIBUTING.md` for the exact type/scope format.
2. **One test file per concern** under `tests/`, not one large `tests.rs` - see
   [argenv](https://github.com/argenv-opencommons/argenv)'s `crates/argenv/tests/*.rs` for the
   pattern this follows.
3. **`cargo fmt` and `cargo clippy --all-targets -- -D warnings` must both be clean** before a PR -
   CI enforces this, but check locally first; `--all-targets` means a lint firing only in a test or
   example still counts.
4. **`cargo doc` must build with no warnings** (`RUSTDOCFLAGS=-D warnings`) - undocumented public
   items are a CI failure, not a style nit.
5. **MSRV is 1.75.0** (`Cargo.toml`'s `rust-version`, `clippy.toml`'s `msrv`, and the CI `msrv` job
   all need to move together if this ever changes). Dependabot is configured to ignore
   `dtolnay/rust-toolchain` for exactly this reason - it can't tell "a version pinned as a floor to
   test against" from "a version to keep current," and will otherwise propose bumping the MSRV job
   to the latest Rust release, silently defeating it. If you ever do want a real MSRV bump, do it by
   hand in all three places, deliberately - never accept a Dependabot PR that touches this pin.
6. **Never edit `CHANGELOG.md` by hand** - `release-plz` generates it from commit history via
   `cliff.toml`. A commit can add a `User-Facing: <text>` footer to control exactly what shows up
   in the changelog line, when the raw commit message isn't the right wording for a release note.

## Releases
Every push to `main` updates one open release PR (version bump + changelog). Merging it cuts the
release (git tag + GitHub Release) and, once `CARGO_REGISTRY_TOKEN` is set as a repo secret,
publishes to crates.io. Nothing else triggers a release - no manual tagging, no separate publish
step. `Cargo.toml` has no `publish = false` here (unlike the rustnix template this was generated
from) - this crate is meant to actually publish once there's something worth releasing.

## Why it's built this way
This template exists so a new project's *process* (CI, releases, test layout) doesn't need to be
re-derived by hand each time, the same way `rustnix`'s Nix devenv already saves you from re-deriving
the IDE setup. The specific choices (release-plz over release-please, PR-title linting over
per-commit linting, `tests/` layout) mirror what's already proven out in
[argenv](https://github.com/argenv-opencommons/argenv), not invented fresh here.
