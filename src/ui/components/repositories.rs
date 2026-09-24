use crate::git::stats::format_size;
use crate::ui::state::{AppState, Focus};
use crate::ui::views;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.layout.repo_table = area;
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let focused = state.focus == Focus::Repositories;
    let visible = views::visible_repos(state);

    let mut title = vec![Span::raw(format!(
        " Repositories {}/{} ",
        visible.len(),
        state.repositories.len()
    ))];
    if state.repo_sort != crate::ui::state::RepoSort::Path {
        title.push(Span::styled(
            format!("· by {} ", state.repo_sort.label()),
            Style::default().fg(theme.text_dimmed),
        ));
    }
    if !state.repo_filter.is_empty() || state.editing_filter {
        title.push(Span::styled(
            format!("· /{} ", state.repo_filter),
            Style::default().fg(theme.primary),
        ));
    }
    if !state.marked_repos.is_empty() {
        title.push(Span::styled(
            format!("· {} marked ", state.marked_repos.len()),
            Style::default().fg(theme.secondary),
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

    if visible.is_empty() {
        let message = if state.is_scanning {
            format!("{} Scanning {}…", state.spinner(), state.root.display())
        } else if state.repositories.is_empty() {
            format!("No Git repository found under {}", state.root.display())
        } else {
            format!("No repository matches \"{}\"", state.repo_filter)
        };
        f.render_widget(
            Paragraph::new(message)
                .style(Style::default().fg(theme.text_dimmed))
                .alignment(Alignment::Center)
                .block(block),
            area,
        );
        return;
    }

    let dim = Style::default().fg(theme.text_dimmed);
    let focused_path = state.focused().map(|r| r.path.clone());
    let rows: Vec<Row> = visible
        .iter()
        .map(|&i| {
            let repo = &state.repositories[i];
            let mark = if state.marked_repos.contains(&repo.path) {
                Span::styled("[x]", Style::default().fg(theme.primary))
            } else {
                Span::styled("[ ]", dim)
            };
            let queued = state.selection.repo(&repo.path).map_or(0, |s| s.len());
            let name = Span::styled(
                state.display_path(&repo.path),
                if queued > 0 {
                    Style::default().fg(theme.merged)
                } else {
                    Style::default().fg(theme.text_normal)
                },
            );

            let branch = if let Some(error) = &repo.error {
                Line::from(Span::styled(
                    format!("⚠ {}", error.lines().next().unwrap_or("error")),
                    Style::default().fg(theme.error),
                ))
            } else if !repo.analyzed {
                Line::from(Span::styled(format!("{} analyzing", state.spinner()), dim))
            } else {
                let mut spans = vec![Span::styled(
                    repo.current_branch
                        .clone()
                        .unwrap_or_else(|| "(detached)".into()),
                    Style::default().fg(theme.secondary),
                )];
                if repo.is_dirty {
                    spans.push(Span::styled(" ●", Style::default().fg(theme.primary)));
                }
                Line::from(spans)
            };

            let cleanable = views::cleanable_branches(repo, &state.config);
            let cleanable = if cleanable > 0 {
                Span::styled(cleanable.to_string(), Style::default().fg(theme.merged))
            } else {
                Span::styled("·", dim)
            };

            let git_size = repo.size_bytes.map(format_size).unwrap_or_default();
            let reclaimable = match (repo.untracked_size_bytes, repo.size_finalized) {
                (Some(bytes), true) if bytes > 0 => format_size(bytes),
                (Some(_), true) => "·".to_string(),
                (Some(bytes), false) => format!("~{}", format_size(bytes)),
                (None, false) if repo.analyzed && repo.error.is_none() => {
                    state.spinner().to_string()
                }
                (None, _) => String::new(),
            };

            Row::new(vec![
                Cell::from(mark),
                Cell::from(name),
                Cell::from(branch),
                Cell::from(Line::from(cleanable).alignment(Alignment::Right)),
                Cell::from(Line::from(git_size).alignment(Alignment::Right)),
                Cell::from(Line::from(reclaimable).alignment(Alignment::Right)),
            ])
        })
        .collect();

    let header = Row::new(["", "Repository", "Branch", "Cln", ".git", "Reclaim"])
        .style(dim.add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(12),
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    });

    let cursor = focused_path.and_then(|path| {
        visible
            .iter()
            .position(|&i| state.repositories[i].path == path)
    });
    state.repo_table.select(cursor);
    f.render_stateful_widget(table, area, &mut state.repo_table);
}
