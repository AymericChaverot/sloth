use crate::git::stats::format_size;
use crate::ui::state::AppState;
use crate::ui::views;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

/// Totals across all repositories and the ones with the most to clean up.
pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let dim = Style::default().fg(theme.text_dimmed);
    let totals = state.totals();
    let now = crate::git::stats::now_ts();
    let stale = state
        .repositories
        .iter()
        .flat_map(|r| r.branches.iter().map(move |b| (r, b)))
        .filter(|(r, b)| {
            crate::cleanup::branch_protection(r, b, &state.config).is_none()
                && crate::cleanup::is_stale(b, &state.config, now)
        })
        .count();
    let dirty = state.repositories.iter().filter(|r| r.is_dirty).count();
    let failed = state
        .repositories
        .iter()
        .filter(|r| r.error.is_some())
        .count();

    let [cards_area, table_area] =
        Layout::vertical([Constraint::Length(5), Constraint::Min(3)]).areas(area);
    let cards = Layout::horizontal([Constraint::Ratio(1, 4); 4]).split(cards_area);

    let mut repo_notes = vec![format!("{dirty} with local changes")];
    if failed > 0 {
        repo_notes.push(format!("{failed} failed to analyze"));
    }
    card(
        f,
        cards[0],
        "Repositories",
        state.repositories.len().to_string(),
        repo_notes.join(" · "),
        theme.secondary,
        theme,
    );
    card(
        f,
        cards[1],
        "Cleanable branches",
        totals.cleanable_branches.to_string(),
        format!(
            "{} merged · {} gone · {stale} stale",
            totals.merged_branches, totals.gone_branches
        ),
        theme.merged,
        theme,
    );
    card(
        f,
        cards[2],
        "Reclaimable",
        format_size(totals.untracked_bytes),
        "untracked & ignored files".into(),
        theme.success,
        theme,
    );
    card(
        f,
        cards[3],
        ".git total",
        format_size(totals.git_bytes),
        format!(
            "{} stashes · {} worktrees",
            totals.stashes, totals.linked_worktrees
        ),
        theme.primary,
        theme,
    );

    let ranked = views::dashboard_rows(state);
    let rows: Vec<Row> = ranked
        .iter()
        .map(|&i| {
            let repo = &state.repositories[i];
            Row::new(vec![
                Cell::from(Span::styled(
                    state.display_path(&repo.path),
                    Style::default().fg(theme.secondary),
                )),
                right(views::cleanable_branches(repo, &state.config).to_string()),
                right(repo.stashes.len().to_string()),
                right(
                    repo.worktrees
                        .iter()
                        .filter(|w| !w.is_main)
                        .count()
                        .to_string(),
                ),
                right(
                    repo.untracked_size_bytes
                        .map(format_size)
                        .unwrap_or_default(),
                ),
                right(repo.size_bytes.map(format_size).unwrap_or_default()),
            ])
        })
        .collect();

    let block = Block::default()
        .title(" Where to start ")
        .title_bottom(Line::from(vec![
            Span::styled(" a ", Style::default().fg(theme.primary)),
            Span::styled("queue every merged & gone branch  ", dim),
            Span::styled("Enter ", Style::default().fg(theme.primary)),
            Span::styled("open repository ", dim),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_active));
    if rows.is_empty() {
        f.render_widget(
            Paragraph::new("Nothing to clean up yet.")
                .style(dim)
                .alignment(Alignment::Center)
                .block(block),
            table_area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from("Repository"),
        right("Branches".into()),
        right("Stashes".into()),
        right("Worktrees".into()),
        right("Reclaim".into()),
        right(".git".into()),
    ])
    .style(dim.add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    state.dashboard_cursor = state.dashboard_cursor.min(ranked.len() - 1);
    state.dashboard_table.select(Some(state.dashboard_cursor));
    f.render_stateful_widget(table, table_area, &mut state.dashboard_table);
}

fn card(
    f: &mut Frame,
    area: Rect,
    title: &str,
    value: String,
    note: String,
    color: Color,
    theme: &crate::ui::theme::Theme,
) {
    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                value,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(note, Style::default().fg(theme.text_dimmed))),
        ])
        .alignment(Alignment::Center)
        .wrap(ratatui::widgets::Wrap { trim: true })
        .block(block),
        area,
    );
}

fn right(text: String) -> Cell<'static> {
    Cell::from(Line::from(text).alignment(Alignment::Right))
}
