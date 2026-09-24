use crate::git::stats::now_ts;
use crate::ui::selection::ItemKind;
use crate::ui::state::{AppState, BranchFilter, BranchSort};
use crate::ui::views;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

/// Every branch of every repository in one table, so branches of several
/// projects can be reviewed and selected together.
pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let dim = Style::default().fg(theme.text_dimmed);
    let rows_index = views::branch_rows(state);

    let mut title = vec![
        Span::raw(format!(" Branches {} ", rows_index.len())),
        Span::styled("· showing ", dim),
        Span::styled(
            state.branch_filter.label(),
            Style::default().fg(theme.primary),
        ),
        Span::styled(" (f) ", dim),
    ];
    if state.branch_sort != BranchSort::Repository {
        title.push(Span::styled(
            format!("· by {} (s) ", state.branch_sort.label()),
            dim,
        ));
    }
    if !state.branch_query.is_empty() || state.editing_filter {
        title.push(Span::styled(
            format!("· /{} ", state.branch_query),
            Style::default().fg(theme.primary),
        ));
    }
    let block = Block::default()
        .title(Line::from(title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_active));

    if rows_index.is_empty() {
        let message = if state.is_analyzing && state.repositories.iter().all(|r| !r.analyzed) {
            format!("{} Analyzing repositories…", state.spinner())
        } else if state.branch_filter == BranchFilter::Cleanable && state.branch_query.is_empty() {
            "Nothing to clean up: no merged or gone branch. Press f to see other branches."
                .to_string()
        } else {
            "No branch matches. Press f to change the filter.".to_string()
        };
        f.render_widget(
            Paragraph::new(message)
                .style(dim)
                .alignment(Alignment::Center)
                .block(block),
            area,
        );
        return;
    }

    let now = now_ts();
    let rows: Vec<Row> = rows_index
        .iter()
        .map(|&(ri, bi)| {
            let repo = &state.repositories[ri];
            let branch = &repo.branches[bi];
            let protected =
                crate::cleanup::branch_protection(repo, branch, &state.config).is_some();
            let mark = if protected {
                Span::styled("[-]", dim)
            } else if state
                .selection
                .contains(&repo.path, ItemKind::Branch, &branch.name)
            {
                Span::styled("[x]", Style::default().fg(theme.merged))
            } else {
                Span::styled("[ ]", dim)
            };
            let mut cells = vec![
                Cell::from(mark),
                Cell::from(Span::styled(
                    state.display_path(&repo.path),
                    Style::default().fg(theme.secondary),
                )),
            ];
            cells.extend(super::details::branch_cells(
                repo,
                branch,
                &state.config,
                theme,
                now,
            ));
            Row::new(cells)
        })
        .collect();

    let header = Row::new(["", "Repository", "Branch", "Status", "Age", "↑↓", "Diff"])
        .style(dim.add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Fill(2),
            Constraint::Fill(3),
            Constraint::Fill(2),
            Constraint::Length(4),
            Constraint::Length(9),
            Constraint::Length(11),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let cursor = state.branch_cursor.min(rows_index.len() - 1);
    let mut table_state = state.branch_table;
    table_state.select(Some(cursor));
    f.render_stateful_widget(table, area, &mut table_state);
    state.branch_table = table_state;
    state.branch_cursor = cursor;
}
