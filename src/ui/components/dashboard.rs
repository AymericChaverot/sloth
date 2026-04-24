use crate::ui::state::AppState;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let mut total_size: u64 = 0;
    let mut total_repos = 0;
    let mut total_branches = 0;
    let mut total_stashes = 0;
    let mut merged_branches = 0;

    for repo in &state.repositories {
        total_repos += 1;
        if let Some(s) = repo.size_bytes {
            total_size += s;
        }
        total_branches += repo.branches.len();
        total_stashes += repo.stashes.len();
        for branch in &repo.branches {
            if branch.is_merged {
                merged_branches += 1;
            }
        }
    }

    let size_str = crate::git::stats::format_size(total_size);

    let content = vec![
        ratatui::text::Line::from(vec![ratatui::text::Span::styled(
            "Sloth Analytics",
            Style::default()
                .fg(theme.primary)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )]),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::raw("Total Repositories: "),
            ratatui::text::Span::styled(
                format!("{}", total_repos),
                Style::default().fg(theme.secondary),
            ),
        ]),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::raw("Total Size on Disk: "),
            ratatui::text::Span::styled(size_str.to_string(), Style::default().fg(theme.secondary)),
        ]),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::raw("Branches: "),
            ratatui::text::Span::styled(
                format!("{}", total_branches),
                Style::default().fg(theme.text_normal),
            ),
            ratatui::text::Span::raw(" ("),
            ratatui::text::Span::styled(
                format!("{} merged", merged_branches),
                Style::default().fg(theme.success),
            ),
            ratatui::text::Span::raw(")"),
        ]),
        ratatui::text::Line::from(vec![
            ratatui::text::Span::raw("Stashes: "),
            ratatui::text::Span::styled(
                format!("{}", total_stashes),
                Style::default().fg(theme.text_normal),
            ),
        ]),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(ratatui::text::Span::styled(
            "Use Left/Right arrows or 'd' to exit the dashboard.",
            Style::default().fg(theme.text_dimmed),
        )),
    ];

    let block = Block::default()
        .title(" Global Dashboard ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_active));

    let p = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ]
            .as_ref(),
        )
        .split(area);

    f.render_widget(p, chunks[1]);
}
