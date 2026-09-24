use crate::engine::Operation;
use crate::git::stats::format_size;
use crate::ui::state::AppState;
use crate::ui::views;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

/// Everything selected for cleanup, in every repository, as it will run.
pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let dim = Style::default().fg(theme.text_dimmed);
    let rows_data = views::queue_rows(state);

    let warnings = rows_data
        .iter()
        .filter(|(repo, op)| {
            state
                .repo(repo)
                .and_then(|r| crate::cleanup::operation_warning(r, op))
                .is_some()
        })
        .count();
    let mut title = vec![Span::raw(" Cleanup queue ")];
    if !rows_data.is_empty() {
        title.push(Span::styled(
            format!("· {} ", state.selection.summary()),
            dim,
        ));
    }
    if warnings > 0 {
        title.push(Span::styled(
            format!("· ⚠ {warnings} need attention "),
            Style::default().fg(theme.primary),
        ));
    }
    let block = Block::default()
        .title(Line::from(title))
        .title_bottom(Line::from(vec![
            Span::styled(" Enter/x ", Style::default().fg(theme.primary)),
            Span::styled("run all  ", dim),
            Span::styled("Space/d ", Style::default().fg(theme.primary)),
            Span::styled("remove  ", dim),
            Span::styled("C ", Style::default().fg(theme.primary)),
            Span::styled("clear ", dim),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_active));

    if rows_data.is_empty() {
        f.render_widget(
            Paragraph::new(vec![
                Line::raw(""),
                Line::raw("The queue is empty."),
                Line::raw(""),
                Line::raw(
                    "Select branches, stashes or worktrees with Space in the Repos or Branches tab,",
                ),
                Line::raw("or press `a` to add every merged or gone branch."),
            ])
            .style(dim)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block),
            area,
        );
        return;
    }

    let mut previous_repo = None;
    let rows: Vec<Row> = rows_data
        .iter()
        .map(|(path, op)| {
            let repo = state.repo(path);
            let repo_cell = if previous_repo == Some(path) {
                Cell::from("")
            } else {
                previous_repo = Some(path);
                Cell::from(Span::styled(
                    state.display_path(path),
                    Style::default().fg(theme.secondary),
                ))
            };
            let (action, item) = match op {
                Operation::DeleteBranch { name, .. } => ("delete branch", name.clone()),
                Operation::DropStash { message, .. } => ("drop stash", message.clone()),
                Operation::RemoveWorktree { path, .. } => (
                    "remove worktree",
                    state.display_path(std::path::Path::new(path)),
                ),
                other => ("", other.describe()),
            };
            let warning = repo
                .and_then(|r| crate::cleanup::operation_warning(r, op))
                .map(|w| Span::styled(format!("⚠ {w}"), Style::default().fg(theme.primary)))
                .unwrap_or_default();
            let size = match op {
                Operation::RemoveWorktree { path, .. } => repo
                    .and_then(|r| r.worktrees.iter().find(|w| &w.path == path))
                    .and_then(|w| w.size_bytes)
                    .map(format_size)
                    .unwrap_or_default(),
                _ => String::new(),
            };
            Row::new(vec![
                repo_cell,
                Cell::from(Span::styled(action, Style::default().fg(theme.error))),
                Cell::from(item),
                Cell::from(warning),
                Cell::from(Line::from(size).alignment(Alignment::Right)),
            ])
        })
        .collect();

    let header = Row::new(["Repository", "Action", "Item", "Warning", "Frees"])
        .style(dim.add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Fill(2),
            Constraint::Length(15),
            Constraint::Fill(3),
            Constraint::Fill(3),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let cursor = state.queue_cursor.min(rows_data.len() - 1);
    let mut table_state = state.queue_table;
    table_state.select(Some(cursor));
    f.render_stateful_widget(table, area, &mut table_state);
    state.queue_table = table_state;
    state.queue_cursor = cursor;
}
