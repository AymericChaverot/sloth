use crate::ui::state::{AppState, UiAction};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let pending = match &state.pending_action {
        Some(a) => a,
        None => return,
    };

    let theme = crate::ui::theme::get_theme(state.theme_index);

    let action_desc = match pending {
        UiAction::CleanRepo => {
            let (b, s, w) = state.selection.counts();
            format!(
                "Delete {} branch(es), {} stash(es), {} worktree(s) in {} repo(s)",
                b,
                s,
                w,
                state.selection.repo_count()
            )
        }
        UiAction::PruneRemotes => {
            let n = if state.selected_repositories.is_empty() {
                1
            } else {
                state.selected_repositories.len()
            };
            format!("Prune remote tracking branches on {} repo(s)", n)
        }
        UiAction::GarbageCollect => {
            let n = if state.selected_repositories.is_empty() {
                1
            } else {
                state.selected_repositories.len()
            };
            format!("Garbage collect {} repo(s)", n)
        }
        UiAction::DeepClean => {
            let n = if state.selected_repositories.is_empty() {
                1
            } else {
                state.selected_repositories.len()
            };
            format!(
                "Deep clean untracked & ignored files in {} repo(s) (nested repos and keep-list spared)",
                n
            )
        }
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Action: ", Style::default().fg(theme.text_dimmed)),
        Span::styled(
            action_desc,
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    if let UiAction::CleanRepo = pending {
        for plan in state.selection.plans(&state.repositories, &state.config) {
            let Some(repo) = state.repositories.iter().find(|r| r.path == plan.repo) else {
                continue;
            };
            lines.push(Line::from(Span::styled(
                repo.path.display().to_string(),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )));
            for op in &plan.operations {
                let mut spans = vec![Span::styled(
                    format!("  - {}", op.describe()),
                    Style::default().fg(theme.error),
                )];
                if let Some(warning) = operation_warning(repo, op) {
                    spans.push(Span::styled(
                        format!("  ⚠ {warning}"),
                        Style::default().fg(theme.primary),
                    ));
                }
                lines.push(Line::from(spans));
            }
            lines.push(Line::raw(""));
        }
    }

    if let UiAction::DeepClean = pending {
        match &state.confirm_preview_lines {
            None => {
                let spinner = state.spinner();
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", spinner), Style::default().fg(theme.primary)),
                    Span::styled(
                        "Computing preview...",
                        Style::default().fg(theme.text_dimmed),
                    ),
                ]));
            }
            Some(preview) => {
                lines.push(Line::from(Span::styled(
                    "Files that will be removed:",
                    Style::default().fg(theme.secondary),
                )));
                for line in preview.iter().take(15) {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", line),
                        Style::default().fg(theme.error),
                    )));
                }
                if preview.len() > 15 {
                    lines.push(Line::from(Span::styled(
                        format!("  ... and {} more", preview.len() - 15),
                        Style::default().fg(theme.text_dimmed),
                    )));
                }
            }
        }
        lines.push(Line::raw(""));
    }

    lines.push(Line::from(vec![
        Span::styled("Press ", Style::default().fg(theme.text_dimmed)),
        Span::styled(
            "Y",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to confirm, ", Style::default().fg(theme.text_dimmed)),
        Span::styled(
            "N",
            Style::default()
                .fg(theme.error)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("/Esc to cancel", Style::default().fg(theme.text_dimmed)),
    ]));

    let block = Block::default()
        .title(" Confirm Action ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(lines).block(block);
    let modal_area = super::centered_rect(55, 70, area);
    f.render_widget(Clear, modal_area);
    f.render_widget(paragraph, modal_area);
}

/// What could be lost by running `op`, if anything.
pub fn operation_warning(
    repo: &crate::git::RepoStatus,
    op: &crate::engine::Operation,
) -> Option<&'static str> {
    use crate::engine::Operation;
    match op {
        Operation::DeleteBranch { name, .. } => repo
            .branches
            .iter()
            .find(|b| &b.name == name)
            .filter(|b| b.has_unique_commits())
            .map(|_| "has commits that are neither merged nor pushed"),
        Operation::RemoveWorktree { force: true, .. } => Some("uncommitted changes will be lost"),
        _ => None,
    }
}
