# rust-boilerplate

Clone it, rename it, start writing Rust. Pure vanilla Nix (no flakes,
no `experimental-features`) + direnv gives you a full toolchain and an
**isolated VSCodium** — pre-configured Rust IDE, zero manual extension
installs — automatically, every time you `cd` into the directory.

<img width="1920" height="1080" alt="rustnix" src="https://github.com/user-attachments/assets/4080efb1-3f66-47ff-aab5-83644a813777" />

## Get started

```bash
git clone <this-repo> my-project
cd my-project
./init_devenv.sh
vscodium .
```

`init_devenv.sh` is the only thing you ever run by hand, and only
once. It trusts `.envrc` (`direnv allow`) and forces an immediate
build/load (`direnv reload`) so the environment is ready the moment
the script finishes — you don't have to `cd` out and back in first.

After that, direnv keeps it loaded automatically. Every time you `cd`
into this directory (in any terminal, forever, no re-running anything):
`rustc`, `cargo`, `clippy`, `rustfmt`, and `vscodium`/`codium` (the
isolated instance) are just... on PATH. Leave the directory and they're
gone again — nothing leaks into your global environment.

## Renaming for your actual project

Three things to change after cloning — nothing else references the
template name:

```
Cargo.toml   → [package] name = "..."
Cargo.toml   → remove the `publish = false` line once this is a real crate you intend to publish
directory    → rename the folder itself, or `git clone ... <new-name>`
```

`publish = false` is there so release-plz's first real release doesn't try to push a
crate named `rust-boilerplate` to crates.io - it's what stops `cargo publish`
outright, with a clear reason, instead of failing on a missing token.

## Use this template

This repo is a GitHub template - "Use this template" (or `gh repo create --template grenudi/rustnix`)
gives you a fresh repo with the same devenv plus CI, release automation, and a test scaffold
already wired up, not just the Nix/VSCodium half. See [`AGENTS.md`](AGENTS.md) for the day-to-day
conventions and [`CONTRIBUTING.md`](CONTRIBUTING.md) for the commit/PR format.

## CI, releases, and tests

- **CI** (`.github/workflows/ci.yml`): format check, clippy (`--all-targets`, warnings as errors),
  `cargo test --workspace --all-features`, `cargo doc` with warnings as errors, and an MSRV job
  pinned to the `rust-version` already set in `Cargo.toml` (1.75.0). Mirrors
  [argenv](https://github.com/argenv-opencommons/argenv)'s CI, minus its project-specific jobs.
- **Releases** (`.github/workflows/release-plz.yml`): [release-plz](https://release-plz.dev/) opens
  and maintains a release PR (version bump + changelog, via `cliff.toml`) on every push to `main`;
  merging it cuts a release. Needs a `CARGO_REGISTRY_TOKEN` repo secret before it can actually
  `cargo publish` - without it, the PR/changelog maintenance still works correctly, only the
  publish step at the end of a merged release needs it.
- **PR titles** (`.github/workflows/pr-title.yml`): linted against Conventional Commits, since
  release-plz reads the squash-merged commit (the PR title) to decide the version bump and
  changelog entry - not every individual commit inside the PR.
- **Tests** (`tests/`): one file per concern (see `tests/sanity.rs`), matching argenv's
  `crates/argenv/tests/*.rs` layout, rather than one large `tests.rs`.

## No `dev` command

Earlier drafts of this had a separate `./dev` launcher script. It's
gone — direnv makes `vscodium` ambient on PATH the moment you `cd`
into the directory, so there's nothing left for a wrapper script to
do. Once `init_devenv.sh` has run once, just:

```bash
vscodium .
```

If you see `command not found`, you're in a shell where direnv's hook
hasn't fired yet for this directory — see the subprocess note in
`init_devenv.sh`'s own output: run `cd .` once, or open a new terminal.

## What "isolated" means

`pkgs.vscode-with-extensions` wraps `codium` with `--extensions-dir`
already pointed at an immutable Nix store path containing exactly the
extensions declared in `nix/extensions.nix` — nothing more, nothing
less, and nothing installed by hand. That path is read-only by design:
**extensions are added by editing `nix/extensions.nix` and reloading,
not via the GUI Install button.** Trying to install something through
the Extensions panel will fail — that's expected, not a bug; it means
your extension set stays fully reproducible from the repo alone.

On top of that, `--user-data-dir` points at `./.devenv/vscodium/user-data`
— project-local and gitignored — so settings, recently-opened, and
workspace state stay per-project without polluting any global
VSCodium/VS Code profile you may have, and without being baked into
the Nix store (unlike the extensions themselves, this needs to stay
writable). Delete `.devenv/` any time to reset that part to a clean
slate; the extension set is unaffected since it was never stored there.

A system-wide `vscodium`/`codium`, if you have one, is shadowed the
moment direnv loads this directory's environment — never the other
way around.

## What's preinstalled

| Extension | Publisher | What it's for |
|---|---|---|
| rust-analyzer | rust-lang | language server: completion, inlay hints, refactors |
| CodeLLDB | vadimcn | debugging — breakpoints, watches, disassembly |
| crates | serayuzgur | inline crate version / upgrade hints in Cargo.toml |
| Even Better TOML | tamasfe | Cargo.toml syntax + formatting |
| **Nix IDE** | **jnoortheen** | **`.nix` language support — wired to nixd + alejandra below** |
| Error Lens | usernamehw | inline diagnostics at the point of error |
| GitLens | eamodio | blame, history, compare |
| Code Spell Checker | streetsidesoftware | catches typos in code/comments |
| direnv | mkhl | makes the editor's integrated terminal direnv-aware too |
| EditorConfig | editorconfig | honours `.editorconfig` if present |

The `.nix` files are the actual backbone of this boilerplate, not an
afterthought — they get the same tier of IDE support as the Rust code:

- **nixd** — the language server. Eval-based, so completions and
  go-to-definition work against the *real* nixpkgs, not just static
  analysis of your own files.
- **alejandra** — the formatter, wired to both `nix.formatterPath`
  (editor's own "Format Document") and `nixd`'s own formatting command
  (so it matches when triggered through the LSP too). Format-on-save
  is on for `.nix` files.
- **statix** / **deadnix** — linting and dead-code detection, on PATH
  for manual runs (`statix check .`, `deadnix .`) — not yet wired to
  inline diagnostics; the nix-ide extension doesn't surface them
  automatically, only nixd's own built-in checks show inline.

`rust-analyzer.server.path` and `nix.serverPath` are both pinned to
the Nix-provided binaries — neither tries to download its own server,
so everything works fully offline once the environment has loaded once.

Toolchain on PATH: `rustc`, `cargo`, `clippy`, `rustfmt`, `cargo-edit`,
`cargo-watch`, `cargo-nextest`, `cargo-audit`, `cargo-outdated`,
`lldb`, `mold` (Linux — faster linking, wired up in
`.cargo/config.toml`), plus `nixd`, `alejandra`, `statix`, `deadnix`.

## Prerequisites (one-time, per machine — not per project)

You need Nix and direnv installed, and direnv's shell hook added to
your shell's rc file. `init_devenv.sh` checks for both and prints
exact install/setup commands if either is missing — you don't need to
hunt for them separately.

## Forcing a reload after changing shell.nix

```bash
./init_devenv.sh
```

Safe and correct to re-run any time — `direnv allow` re-confirms trust
if `.envrc`'s content changed, and `direnv reload` forces the
environment to recompute right now rather than waiting for the next
`cd`. This is exactly what "reevaluate if already enabled" means:
without the explicit `reload`, an already-trusted, unchanged `.envrc`
is served from direnv's cache and nothing happens until you leave and
re-enter the directory.

## Pinning for full reproducibility

Out of the box, `nix/nixpkgs-pin.nix` fetches the current tip of
nixpkgs' `nixos-24.05` branch — zero setup, always works, but not
byte-for-byte reproducible since the branch moves. To freeze to an
exact commit + verified hash:

```bash
./nix/update-pins.sh
```

Paste the printed values into `nix/nixpkgs-pin.nix`. If
`nix/extensions.nix` has any `lib.fakeHash` placeholders left (a
marketplace extension not yet resolved), run:

```bash
nix-shell --run true
```

It fails on the first unresolved extension and prints the correct
hash in the error message — standard Nix behaviour, not a bug. Paste
it in, re-run, repeat until it succeeds.

## Adding or changing extensions

Edit `nix/extensions.nix`. Anything in nixpkgs' own curated set
(`pkgs.vscode-extensions.<publisher>.<name>`) needs no hash — just add
the line. Anything else goes in the `extensionsFromVscodeMarketplace`
block with `lib.fakeHash` as a placeholder; see "Pinning" above for
how the real hash gets filled in.
