//! kantui binary internals exposed as a library so integration tests can
//! import them. The binary's `main.rs` is a thin wrapper over these modules.

pub mod action;
pub mod app;
pub mod cli;
pub mod config;
pub mod controller;
pub mod event;
pub mod jump;
pub mod keybinds;
pub mod keymap;
pub mod logging;
pub mod state;
pub mod tui;
pub mod view;
