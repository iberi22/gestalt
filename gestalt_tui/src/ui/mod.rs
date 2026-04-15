//! UI module — composes all TUI widgets

pub mod agents;
pub mod header;
pub mod input;
pub mod output;
pub mod status;

pub use agents::draw_agents;
pub use header::draw_header;
pub use input::draw_input;
pub use output::draw_output;
pub use status::draw_status;
