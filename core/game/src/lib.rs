//! The roguelite layer of Chrogue and its JSON command protocol.
//!
//! - `content`: relics, upgrades, floors, and prices as data.
//! - `run`: the data of a run and of the permanent progress, and the life of a run.
//! - `battle`: one battle of a run on the chess engine.
//! - `save`: saved data through a `Storage`.
//! - `session`: the screens and `Session::command`, the one entry point of the protocol.
//! - `view`: the data that a client draws.
//! - `chess`: the one module that calls the engine.
//!
//! The crate has no I/O except `save::FileStorage`. `PROTOCOL.md` documents the protocol.

pub mod battle;
pub mod chess;
pub mod content;
pub mod protocol;
pub mod random;
pub mod run;
pub mod save;
pub mod session;
pub mod view;

pub use save::{Doc, FileStorage, MemoryStorage, Storage};
pub use session::{Screen, Session};
