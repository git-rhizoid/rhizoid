//! One module per verb. argenv declares each command's named flags only -
//! it has no concept of a subcommand or a positional argument (confirmed by
//! reading its actual source; see the design log's
//! `argenv-is-flags-and-env-only` entry) - so the verb itself and each
//! command's one positional (a repo spec, a fork URL, a module name) are
//! read directly from argv by `main`, before the remaining tokens are handed
//! to argenv to resolve.
//!
//! **v1 convention, chosen for simplicity over flexibility:** a command's
//! positional, if it has one, must be the first token after the verb -
//! `rhizoid add owner/repo --org foo`, not `rhizoid add --org foo
//! owner/repo`. Revisit if this turns out to matter.

pub mod add;
pub mod import;
pub mod init;
pub mod refresh;
pub mod remove;
pub mod status;
pub mod update;
