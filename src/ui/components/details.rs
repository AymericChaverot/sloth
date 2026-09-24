use crate::git::stats::{format_age, format_size, now_ts};
use crate::ui::state::{AppState, Focus};
use crate::ui::theme::Theme;
use crate::ui::views::{self, DetailRow};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let focused = state.focus == Focus::Details;
    let dim = Style::default().fg(theme.text_dimmed);

    let Some(repo) = state.focused() else {
        f.render_widget(
            Block::default().borders(Borders::ALL).title(" Details "),
            area,
        );
        return;
    };

    let mut title = vec![Span::styled(
        format!(" {} ", state.display_path(&repo.path)),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    if let Some(url) = &repo.remote_url {
        title.push(Span::styled(format!("· {url} "), dim));
    }
    let queued = state.selection.repo(&repo.path).map_or(0, |s| s.len());
    if queued > 0 {
        title.push(Span::styled(
            format!("· {queued} queued "),
            Style::default().fg(theme.merged),
        ));
    }
    let block = Block::default()
        .title(Line::from(title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused {
            theme.border_active
        } else {
            theme.border
        }));

    let message = if let Some(error) = &repo.error {
        Some((format!("Analysis failed: {error}"), theme.error))
    } else if !repo.analyzed {
        Some((
            format!("{} Analyzing repository…", state.spinner()),
            theme.text_dimmed,
        ))
    } else {
        None
    };
    if let Some((text, color)) = message {
        f.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(color))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(block),
            area,
        );
        return;
    }

    let now = now_ts();
    let rows: Vec<Row> = views::detail_rows(repo)
        .into_iter()
        .map(|row| {
            let (kind, id) = row.item();
            let protection = row.protection(repo, &state.config);
            let mark = if protection.is_some() {
                Span::styled("[-]", dim)
            } else if state.selection.contains(&repo.path, kind, id) {
                Span::styled("[x]", Style::default().fg(theme.merged))
            } else {
                Span::styled("[ ]", dim)
            };
            let mut cells = vec![Cell::from(mark)];
            cells.extend(match row {
                DetailRow::Branch(b) => branch_cells(repo, b, &state.config, theme, now),
                DetailRow::Stash(s) => [
                    Cell::from(Line::from(vec![
                        Span::styled("stash ", dim),
                        Span::raw(s.message.clone()),
                    ])),
                    Cell::from(""),
                    Cell::from(age(s.created_ts, now)),
                    Cell::from(""),
                    Cell::from(""),
                ],
                DetailRow::Worktree(w) => {
                    let mut status = Vec::new();
                    if let Some(p) = protection {
                        status.push(badge(p.label(), theme.text_dimmed));
                    }
                    if w.is_dirty {
                        status.push(badge("dirty", theme.primary));
                    }
                    if w.is_locked {
                        status.push(badge("locked", theme.text_dimmed));
                    }
                    if w.is_prunable {
                        status.push(badge("missing", theme.error));
                    }
                    let path = state.display_path(std::path::Path::new(&w.path));
                    let name = match &w.branch {
                        Some(b) => format!("{path} [{b}]"),
                        None => path,
                    };
                    [
                        Cell::from(Line::from(vec![
                            Span::styled("worktree ", dim),
                            Span::raw(name),
                        ])),
                        Cell::from(Line::from(status)),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(
                            Line::from(w.size_bytes.map(format_size).unwrap_or_default())
                                .alignment(Alignment::Right),
                        ),
                    ]
                }
            });
            Row::new(cells)
        })
        .collect();

    let row_count = rows.len();
    let header = Row::new(["", "Name", "Status", "Age", "↑↓", "Diff"])
        .style(dim.add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
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

    // The table borrows the repository from the state: render with a copy of
    // the table state, then store it back.
    let detail_index = state.detail_index.min(row_count.saturating_sub(1));
    let mut table_state = state.detail_table;
    table_state.select((focused && row_count > 0).then_some(detail_index));
    f.render_stateful_widget(table, area, &mut table_state);
    state.detail_table = table_state;
    state.detail_index = detail_index;
}

/// Name, status, age, ahead/behind and diff cells of a branch row.
pub(crate) fn branch_cells<'a>(
    repo: &crate::git::RepoStatus,
    b: &'a crate::git::models::BranchInfo,
    config: &crate::config::Config,
    theme: &Theme,
    now: i64,
) -> [Cell<'a>; 5] {
    let dim = Style::default().fg(theme.text_dimmed);
    let protection = crate::cleanup::branch_protection(repo, b, config);
    let mut status = Vec::new();
    if protection.is_none() && crate::cleanup::is_stale(b, config, now) {
        status.push(badge("stale", theme.text_dimmed));
    }
    if let Some(p) = protection {
        status.push(badge(p.label(), theme.text_dimmed));
    }
    if b.is_merged {
        status.push(badge("merged", theme.merged));
    } else if b.is_squash_merged {
        status.push(badge("squashed", theme.merged));
    }
    if b.is_dead {
        status.push(badge("gone", theme.error));
    } else if b.upstream.is_none() {
        status.push(badge("local", theme.text_dimmed));
    } else if b.upstream_ahead > 0 {
        status.push(badge(
            &format!("{} unpushed", b.upstream_ahead),
            theme.primary,
        ));
    }
    if b.has_unique_commits() && protection.is_none() {
        status.push(badge("⚠", theme.primary));
    }

    let colored = |n: usize, color| {
        if n > 0 {
            Style::default().fg(color)
        } else {
            dim
        }
    };
    let divergence = if repo.default_branch.as_deref() == Some(b.name.as_str()) {
        Line::from("")
    } else {
        Line::from(vec![
            Span::styled(format!("↑{}", b.ahead), colored(b.ahead, theme.success)),
            Span::raw(" "),
            Span::styled(format!("↓{}", b.behind), colored(b.behind, theme.error)),
        ])
    };
    let changes = if b.diff_insertions + b.diff_deletions > 0 {
        Line::from(vec![
            Span::styled(
                format!("+{}", b.diff_insertions),
                Style::default().fg(theme.success),
            ),
            Span::raw(" "),
            Span::styled(
                format!("-{}", b.diff_deletions),
                Style::default().fg(theme.error),
            ),
        ])
    } else {
        Line::from("")
    };

    let name_style = if protection.is_some() {
        dim
    } else {
        Style::default().fg(theme.text_normal)
    };
    [
        Cell::from(Span::styled(b.name.as_str(), name_style)),
        Cell::from(Line::from(status)),
        Cell::from(age(b.last_commit_ts, now)),
        Cell::from(divergence.alignment(Alignment::Right)),
        Cell::from(changes.alignment(Alignment::Right)),
    ]
}

fn badge(text: &str, color: ratatui::style::Color) -> Span<'static> {
    Span::styled(format!("{text} "), Style::default().fg(color))
}

fn age(ts: Option<i64>, now: i64) -> Line<'static> {
    Line::from(ts.map(|ts| format_age(ts, now)).unwrap_or_default()).alignment(Alignment::Right)
}
