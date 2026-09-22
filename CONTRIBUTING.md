# Contributing

## PR titles: Conventional Commits

PR titles are linted (`pr-title.yml`) against [Conventional Commits](https://www.conventionalcommits.org/):
`type(scope): subject`, lowercase, imperative mood. `release-plz` reads the squash-merged commit
(the PR title) to decide the next version and write the changelog entry, so the type you pick has
a real effect - the same mapping as [argenv](https://github.com/argenv-opencommons/argenv):

| Type | Use for | Version bump |
|---|---|---|
| `feat` | New functionality | minor |
| `fix` | A bug fix | patch |
| `perf` | A performance improvement with no behavior change | patch |
| `refactor` | Internal reshaping, no functional or behavior change | patch, no changelog entry |
| `docs` | Documentation only | patch, no changelog entry |
| `style` | Formatting only, no code meaning change | no bump |
| `test` | Test-only changes | no bump |
| `build` | Build system or dependency changes | no bump |
| `ci` | Workflow changes | no bump |
| `chore` | Housekeeping | no bump |

A breaking change adds `!` after the type/scope (`feat!: ...`) or a `BREAKING CHANGE:` footer, and
bumps major.

Scope is optional. This template ships with no fixed scope list (a fresh repo has no established
parts yet) - once the project's shape settles, add one to `.github/workflows/pr-title.yml`'s
`scopes:` list, the way argenv's does (`model`, `cli`, `contract`, ...).

## Changelog wording

The changelog is generated from commit messages via `cliff.toml`. If the raw commit message isn't
the right wording for a release note, add a footer:
```
feat(parser): rewrite the tokenizer for streaming input

User-Facing: parsing large files no longer loads the whole input into memory first.
```
The `User-Facing:` line is what shows up in `CHANGELOG.md`; the rest is developer-facing context.

## Workflow

See [`AGENTS.md`](AGENTS.md) for setup and the day-to-day conventions (test layout, MSRV,
lint/doc requirements). In short: `cargo fmt` and `cargo clippy --all-targets` clean, tests added
under `tests/` per concern, and a properly typed PR title - everything else is what CI checks.
