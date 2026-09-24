//! Command-line subcommands that run without the TUI.

mod clean;
mod restore;

pub use clean::{CleanArgs, clean};
pub use restore::restore;
