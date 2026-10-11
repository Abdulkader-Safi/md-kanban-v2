//! Markdown for mdk.
//!
//! - [`commands`]: the formatting toolbar's text edits. Plain text in, plain text out.
//! - [`source`]: what the preview does to markdown before it is parsed, and how links and
//!   wikilinks find their cards. Also plain text.
//! - [`preview`]: the rendered preview, as a gpui-kit `TextView` with mdk's plugins.
//!
//! None of it knows about the board.

pub mod commands;
pub mod preview;
pub mod source;
