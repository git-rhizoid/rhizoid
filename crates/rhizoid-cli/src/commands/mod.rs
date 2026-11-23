//! One module per command (the cutting verbs share [`cutting`]). The command and its target are read by argenv (`cli.rs`); a command here gets them as
//! values, never as arguments.

pub mod assemble;
pub mod bump;
pub mod cutting;
pub mod follow;
pub mod send_back;
pub mod status;
