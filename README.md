# rhizoid

Fork, own, and sync git dependencies instead of relying on a package manager. Each dependency
becomes a real, independently-owned GitHub fork (a **rhizoid-cutting**, or **rhizcut** for short) -
pinned, patchable without waiting on an upstream PR, and able to send clean patches back upstream
when you want to. See [`git-rhizoid/git-rhizoid.github.io`](https://github.com/git-rhizoid/git-rhizoid.github.io)
for the design log and current status.

**Status: `rhizoid status`, `rhizoid assemble` and `rhizoid send-back` work** - status is the first step of the follow tool for every edge a
repository has (a slice, a tool pin, a fork, an integration); assemble merges a named list of branches onto a base into
a tag, in any repository; send-back drafts the next series version of a branch for its upstream; `add`, `import`, `update` and
`refresh` wait for the GitHub adapter. The rest of the repo is
the devenv and CI/release scaffolding (from [rustnix](https://github.com/grenudi/rustnix)).

## rhizoid status

```
rhizoid status [--graph FILE] [--root DIR] [--fetch] [--json]
```

One read-only report over the edges a graph file names (`examples/linux-phone-oco.graph.toml` is one): which
repositories follow which source, and what a source's tags look like. Each follower's own pin files, read at
`origin/main` of its clone under `--root` (`~/Projects`), say the rest. Every check is `current`, `behind` or
`moved-by-hand`, and how; it ends 0 when nothing is drift, 1 when something is, 2 when a clone or remote could not be
read. `tests/agree-with-graph-drift.sh` runs it against the shell it replaces, `graph-drift.sh` of
`linux-phone-oco/docs`, on the real graph and compares them edge by edge.

It reads the clones as they are, and asks remotes only for their refs (`git ls-remote`: no objects - a fork's
upstream, an integration's `origin` and `releases`) (`--fetch` first fetches each clone's branches from `origin`, pruning the ones `origin`
no longer has, then its tags without pruning, so a tag made locally stays), and pin files as TOML, not as lines. Where it answers differently from the shell on purpose: a file named in `own` that neither side has is
`behind` (a typo), a pin that names a release its source never made is `behind`, a fork ahead of its upstream is
`moved-by-hand` and not behind, a pin file that is not what it should be stops the report. The old `status`, which
listed the modules of `rhizoid.toml` and said drift was unknown, is gone: a cutting returns as an edge of the graph
when cuttings are built (step 4 of the order).

**An integration edge** (`[[integration]]`) reads one repository whose branches `rhizoid assemble` merges into
tags, against what the graph gives it - every name is the graph's, none is rhizoid's:

```toml
[[integration]]
repo = "acme/app"
release_prefix = "v"                     # release tags: this, then a digit (v1.4, v1.5-rc2); always in the scheme
releases = "acme/upstream"               # where releases come from: its tags, read with git ls-remote (no objects)
topics = ["feature/ITEM", "fix/ITEM/BASE"]   # the branches lists merge; BASE says what one is based on
assembled = "candidate/LIST/N"           # the shape of the tags assemble makes here
scheme = { branches = ["main", "feature/ITEM", "fix/ITEM/BASE"], tags = ["candidate/LIST/N"] }
```

A pattern is `/`-separated parts, each a literal with a CAPITAL placeholder at its end (`N`: digits; any other: one
part's text). [`examples/kernel.graph.toml`](examples/kernel.graph.toml) is linux-phone-oco's kernel clone as one
example. Each check runs only when the edge gives what it needs:

| check | needs | current | otherwise |
|---|---|---|---|
| `name-scheme` | `scheme` | every local branch and tag fits it | `moved-by-hand`: one finding for the branches outside it and one for the tags, the first ten named and the rest counted |
| `topic-base` | `topics`, and `releases` for the newest | a topic is on the newest release of `releases` (a stable point release counts, a release candidate does not), or on the BASE its name says | `behind` (the release it is on; the newest may not be in the clone yet), `moved-by-hand` (off its BASE); `not-known-yet` when no `releases` is named |
| `branch-origin` | `topics` | every local topic branch is `origin`'s, asked of `origin` itself (`git ls-remote`), so a clone that tracks only some branches still gets an answer | `behind`: assemble would take the stale local one (or `origin`'s commit is not fetched yet); `moved-by-hand`: commits `origin` lacks, diverged, or a topic only in this clone; `not-known-yet` when `origin` cannot be asked |
| `recipe` | - | the newest tag of each assembled list and base: its recipe, read back from its message, names every branch where it is now | `behind`: a branch moved or is gone, so assemble again; `moved-by-hand`: a tag of the `assembled` shape with no recipe (made by hand) |
| `worktree-tag` | - | each worktree assemble locked has its tag's tree and no change | `moved-by-hand`: a different tree, a change, or a tag that was never made; `behind`: an unlocked worktree in a `rhizoid` home, left at a conflict |

A fork and its upstream stay a `[[fork]]` edge. What the clone cannot say (no release tag in it, a topic with no
release under it, a worktree folder that cannot be read) is `not-known-yet`, which is not drift. Worktrees are read
with `git worktree list -z` (git 2.36 or later), and only the ones assemble locked are read for changes, without
taking a lock.

## rhizoid assemble

```
rhizoid assemble LIST --lists FILE [--repo DIR] [--worktree DIR] [--reproduces COMMIT] [--fetch] [--json]
rhizoid assemble --done TAG [--repo DIR] [--force]
```

Merges a named list of branches, in order, onto a base in a fresh worktree, and tags the result - in any
repository (`--repo`, else here). The list file (`--lists`, or `RHIZOID_LISTS`; there is no default) is rhizoid's
format, and each user keeps its own: every name in it is the file's.

```toml
name = "app"                        # optional: {name} in the tag (`device` is read as name, from files written before)
tag = "candidate/{list}/{n}"        # optional: {name}, {list}, {base} and one {n}; default int/{name}/{list}/{base}/{n}
                                    #   (int/{list}/{base}/{n} without a name)
branches = ["feature/ITEM", "fix/ITEM/BASE"]   # optional: the branches a list may name; with none, any branch

[lists.release]                     # named lists: one tag each
base = "v1.4"                       # a tag, or a commit

[[lists.release.entries]]           # merged in this order
branch = "feature/login"
for = "the new login"               # what it is for
state = "known-good"                # known-good (with test_run and tested_commit) or trying
test_run = "ci-2026-11-20"
tested_commit = "29d92793b"         # a commit's sha (or a prefix of one): never HEAD, a branch or a tag, which move
```

[`examples/kernel-topics.toml`](examples/kernel-topics.toml) is linux-phone-oco's kernel lists as one example. A
branch is the local one, else `origin`'s (`entry.N.source=local|origin`); a local branch wins even when `origin`'s has
moved on, and `rhizoid status` flags it (`branch-origin`). The name, the list name and the base must each fit in one
part of a tag name; anything else in the file is refused before anything is made.

**What it does**, in order: reads the base, every branch, each known-good entry's tested commit (resolved, as a sha,
to its full sha) and the `--reproduces` commit, and stops if one is missing. When the base and every entry (its
branch's commit, what it is for, its state) equal an existing tag's recipe, that tag is the result: it prints it with
`unchanged=yes`, makes no tag, and reuses the tag's worktree if one is left (`--worktree` is then refused: it asks for
a fresh one). Otherwise it numbers the tag one past the highest of its shape, makes a fresh worktree detached at the
base in `${AGENT_WORKTREES:-~/.local/share/agent-worktrees}/REPO/rhizoid/` (REPO is the main clone's folder name; the
tag with `-` for `/`, or `--worktree`, which must be inside that folder - anywhere else is refused), turns on the
clone's shared rerere memory (`rerere.enabled` in its config: a person's resolution is recorded too), and merges each
branch with `git merge --no-ff`, rerere replaying what it knows. How each entry went in is `entry.N.merged`: `merged`,
`rerere-resolved` (its conflict replayed from a recorded resolution) or `already-in` (the base or an earlier entry held
it; nothing merged). Any other conflict stops the run: it names the entry and the earlier entries that changed the
same paths, and leaves the worktree mid-merge and unlocked. Resolve it there and commit, and rerere remembers the
resolution, so the next run gets through; then `git worktree remove` it. When every branch is merged, it writes an
annotated tag whose message carries the recipe (the base and its sha, every entry with its sha), and locks the
worktree (`git worktree lock --reason`), so several stay alive until each is ended. `--reproduces COMMIT` then compares
the tag's tree with the commit's and says `equal`, or lists the paths that differ. `--fetch` first fetches every
remote, pruning branches they no longer have (never tags: an assembled tag is made locally).

**Worktrees end** with `rhizoid assemble --done TAG`: it removes the worktree assemble locked for TAG, refused when the
worktree has changes (or cannot be read) unless `--force`. The tag stays. A large tree is gigabytes checked out, so end
one as soon as what was built from it is built.

**Its report** is `key=value` lines, or the same as JSON with `--json`; a field added later is a new key:

```
name=app
list=release
base=v1.4
base_sha=...
n=1
entry.1.branch=feature/login
entry.1.sha=...
entry.1.source=local
entry.1.for=the new login
entry.1.state=known-good
entry.1.test_run=ci-2026-11-20
entry.1.tested_commit=29d92793b...  (the full sha)
entry.1.merged=merged
feature.login=...                   (one per entry: its branch with . for /, = its sha)
tested=...                          (the known-good entries' tested commits, space-separated)
worktree=/home/you/.local/share/agent-worktrees/app/rhizoid/candidate-release-1
tag=candidate/release/1
tag_sha=...
commit=...
```

`unchanged=yes` when an existing tag was the result. At a conflict: `conflict.entry`, `conflict.with`
(space-separated) and `conflict.path.N`, and no tag. With `--reproduces`: `reproduces.commit`,
`reproduces=equal|differs` and `reproduces.path.N`. `--done` prints `done=TAG` and `removed=PATH`. It ends 0 when tagged
(or unchanged) and equal, 1 at a conflict or a difference, 2 when it could not run. rhizoid knows nothing of its
users' records: a caller reads the report (`base_sha`, `tag`, `tag_sha`, the per-branch lines and `tested` are what a
test-run record takes) and keeps its own record checks.

## rhizoid send-back

```
rhizoid send-back BRANCH --onto REF --out DIR [--config FILE] [--tested SHA] [--fork ID] [--repo DIR] [--json]
```

Drafts the next series version of a branch for its upstream, in any repository. It never signs, re-authors, sends
or pushes, and claims nothing in the cover letter: the author reads the draft, signs and sends.

**Versions and drafts.** A version exists once it is sent. Numbering counts only the versions sent - the tags
`{branch}/v{n}` - and a draft is one past them. A draft has its own tag (`{branch}/vN-draft`) and send branch
(`send/{leaf}/vN`); running it again for the same N, after a fix, replaces that tag, that branch and that draft's files in
`--out` (`replaced=yes`), because none of them was ever pushed. The new draft is cut beside the earlier one and
swapped in only when ready, so a re-run that fails leaves the earlier draft as it was; only the draft's own files
(SEND-NOTES and its `vN-*.patch`) are replaced, and a send branch someone worked on, a tag rhizoid did not make or a
worktree in use is refused, never replaced. **The one manual step**: once the author has
sent version N from his signed branch, he marks it sent:

```
git tag topic/ITEM/vN <signed tip>
```

and the next draft is vN+1, its range-diff against vN.

**What it does**, in order: reads the branch (local, else `origin`'s), the target (`--onto`, any ref of the tree the
series is for: a maintainer's `for-next`, a `next-YYYYMMDD` tag), the tested commit (`--tested`, a commit's sha, printed
in full; never HEAD or a branch), and refuses an `--out` folder that is neither empty nor this same draft's. It
refuses, before writing anything, when the clone's history cannot hold what the draft needs - where the branch and the
target meet, the version last sent - naming what is missing and how to get it (a shallow clone: `git fetch
--deepen=N` or `--shallow-since=DATE`, or fetching the ref): a draft never carries a wrong base-commit. It rebases the
branch onto the target in a fresh worktree on the send branch, authorship kept, nothing signed, no other branch moved
(`--no-update-refs`), merges flattened. A conflict stops it: it names the commit it could not replay and the paths, and
keeps nothing; a branch the target already holds is nothing to send, and commits the target already has are counted
(`dropped=N`). Then `git format-patch --cover-letter -vN --base` cuts the series into `--out`, with `--range-diff`
against the version last sent, and under the cover letter's blurb a changelog skeleton (`Changes since vN-1:`) and the
testing line - a marked blank (`Testing: <to fill>`, or the config's), never filled from commit messages. The
configured checks run in order in the rebased tree, then the recipients command (its stdout lines are the recipients;
a failing one fails the run). A person's `format.*` config (signature, to, cc, headers, from, suffix, notes) does not
reach the series. `SEND-NOTES` beside the patches holds the draft, the base, the tested commit and every check's exit
code and output. When every counted check passes, the draft is tagged and its send branch kept; when one fails, the
series stays to be read, and nothing else. The worktree always goes, and a run that cannot finish keeps nothing.

**The config** (`--config`, or `RHIZOID_CONFIG`; none: the default names and no checks) is rhizoid's own file. Its
`[send]` section names everything ([`examples/kernel-send.toml`](examples/kernel-send.toml) is a Linux kernel tree's,
as one example, its draft's expected checkpatch findings reported and not counted):

```toml
[send]
branch = "send/{leaf}/v{n}"         # the send branch: {branch}, {leaf} (its last part), one {n}; the default
draft_tag = "{branch}/v{n}-draft"   # a draft's tag; the default
tag = "{branch}/v{n}"               # a version sent - the author tags it; drafts are numbered past it; the default
testing = "Testing: <to fill>"      # the cover letter's testing line, a marked blank; the default

[[send.check]]                      # in order; an exit but 0 fails the draft
name = "lint"
run = ["tools/lint", "--strict"]    # argv, never a shell; its exceptions are its own arguments
patches = true                      # append the patch files (the cover letter left out)

[[send.check]]
name = "lint-all"
run = ["tools/lint"]
patches = true
fails = false                       # its findings are reported in SEND-NOTES and count for nothing

[send.recipients]                   # each line it prints is a recipient
name = "owners"
run = ["tools/owners"]
patches = true
```

**Its report** is `key=value` lines, or the same as JSON with `--json`. `carried-in=ID/BRANCH` (with `--fork ID`),
`local-ref` (the branch) and `file` (the `--out` folder, as given) are an upstream item's fields as they are; a draft
prints no `version` - it is `draft=vN`, with `draft_tag` once made. Beside them `branch`, `branch_sha`, `base`,
`base_sha`, `tested`, `send_branch`, `replaced`, `patch.N`, `dropped`, `range_diff` (the version sent it is compared
with, or `none`), `check.N.name`, `check.N.exit` and `check.N.counts=no` for a reported-only check, `recipient.N`, and at
a conflict `conflict.commit`, `conflict.subject` and `conflict.path.N`. It ends 0 when the draft is tagged, 1 at a
conflict or a failed check, 2 when it could not run.

Install at a release: the x86_64 Linux binary is attached to every release, with its sha256 beside it
(`.github/workflows/assets.yml`):

```
curl -fsSLO https://github.com/git-rhizoid/rhizoid/releases/download/vX.Y.Z/rhizoid-x86_64-unknown-linux-gnu.sha256
curl -fsSLO https://github.com/git-rhizoid/rhizoid/releases/download/vX.Y.Z/rhizoid-x86_64-unknown-linux-gnu
sha256sum -c rhizoid-x86_64-unknown-linux-gnu.sha256 && install -m 755 rhizoid-x86_64-unknown-linux-gnu ~/.local/bin/rhizoid
```

or from source: `cargo install --git https://github.com/git-rhizoid/rhizoid --tag vX.Y.Z rhizoid-cli`.

The rest of this README documents the devenv this project is built with - Nix + direnv giving a
full Rust toolchain and an isolated, pre-configured VSCodium automatically, every time you `cd`
into the directory.

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

## CI, releases, and tests

Inherited from [rustnix](https://github.com/grenudi/rustnix), which this repo was generated from -
see that template's own README/AGENTS.md for the full rationale. In short: PR titles are linted as
Conventional Commits, `release-plz` maintains a release PR (changelog + version bump) on every push
to `main`, and tests live under `tests/`, one file per concern.

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

## Licence

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE). Every file
is covered by an annotation in [`REUSE.toml`](REUSE.toml) (or an SPDX header), and `bin/check-licences.sh` fails CI
on a file that is not.
