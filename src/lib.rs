//! TUI terminal multiplexer (tmux-like) in Rust.
//!
//! Manages multiple terminal panes backed by PTYs, rendered with `ratatui`
//! and parsed with `vt100`.
//!
//! # Module overview
//!
//! - [`app`] — application state, action enum, and centralized mutations
//! - [`input`] — keyboard event → [`app::Action`] mapping per mode
//! - [`pane`] — single pane: wraps a `portable-pty` pair and a `vt100::Parser`
//! - [`ui`] — ratatui rendering and vt100 → ratatui color conversion
//! - [`tree`] — pane recursive tree module for resizing

pub mod app;
pub mod input;
pub mod pane;
pub mod tree;
pub mod ui;
