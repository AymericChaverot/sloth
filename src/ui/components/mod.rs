pub mod branches;
pub mod confirm_modal;
pub mod dashboard;
pub mod details;
pub mod diff_modal;
pub mod execution_modal;
pub mod graph;
pub mod header;
pub mod help;
pub mod queue;
pub mod repositories;
pub mod tabs;

use ratatui::layout::{Constraint, Flex, Layout, Rect};

/// A rectangle of the given percentage size, centered in `area`.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [area] = Layout::vertical([Constraint::Percentage(percent_y)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .areas(area);
    area
}
