//! One module per command that does something: the rest wait for the GitHub adapter and say so
//! ([`unbuilt`]). The command and its target are read by argenv (`cli.rs`); a command here gets them as
//! values, never as arguments.

pub mod assemble;
pub mod init;
pub mod remove;
pub mod send_back;
pub mod status;
pub mod unbuilt;
