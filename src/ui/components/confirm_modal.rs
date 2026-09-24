use crate::ui::state::{AppState, UiAction};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Asks for confirmation before anything is deleted, showing what will be.
pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let Some(pending) = &state.pending_action else {
        return;
    };
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let dim = Style::default().fg(theme.text_dimmed);
    let targets = state.action_targets().len();

    let headline = match pending {
        UiAction::CleanRepo => format!("Delete {}", state.selection.summary()),
        UiAction::PruneRemotes => {
            format!("Prune dead remote-tracking branches in {targets} repo(s)")
        }
        UiAction::GarbageCollect => format!("Garbage collect {targets} repo(s)"),
        UiAction::DeepClean => format!(
            "Delete untracked and ignored files in {targets} repo(s) \
             (nested repositories and `deep_clean_keep` files are kept)"
        ),
    };

    let mut body: Vec<Line> = Vec::new();
    match pending {
        UiAction::CleanRepo => {
            for plan in state.selection.plans(&state.repositories, &state.config) {
                let Some(repo) = state.repo(&plan.repo) else {
                    continue;
                };
                body.push(Line::from(Span::styled(
                    state.display_path(&repo.path),
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )));
                for op in &plan.operations {
                    let mut spans = vec![Span::raw(format!("  {}", op.describe()))];
                    if let Some(warning) = crate::cleanup::operation_warning(repo, op) {
                        spans.push(Span::styled(
                            format!("  ! {warning}"),
                            Style::default().fg(theme.primary),
                        ));
                    }
                    body.push(Line::from(spans));
                }
            }
        }
        UiAction::DeepClean => match &state.confirm_preview_lines {
            None => body.push(Line::from(Span::styled(
                format!("{} Listing files…", state.spinner()),
                dim,
            ))),
            Some(preview) => {
                body.extend(preview.iter().map(|l| Line::raw(format!("  {l}"))));
            }
        },
        UiAction::PruneRemotes | UiAction::GarbageCollect => {
            for path in state.action_targets() {
                body.push(Line::raw(format!("  {}", state.display_path(&path))));
            }
        }
    }

    let modal = super::centered_rect(70, 70, area);
    f.render_widget(Clear, modal);
    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.secondary));
    let inner = block.inner(modal);
    f.render_widget(block, modal);

    let [headline_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            headline,
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        )))
        .wrap(Wrap { trim: true }),
        headline_area,
    );

    // Long lists are cut with a pointer to where the full list lives.
    let room = body_area.height as usize;
    if body.len() > room && room > 0 {
        let hidden = body.len() - (room - 1);
        body.truncate(room - 1);
        let hint = if matches!(pending, UiAction::CleanRepo) {
            format!("  … {hidden} more lines — see the Queue tab")
        } else {
            format!("  … {hidden} more")
        };
        body.push(Line::from(Span::styled(hint, dim)));
    }
    f.render_widget(Paragraph::new(body), body_area);

    f.render_widget(
        Paragraph::new(vec![
            Line::raw(""),
            Line::from(vec![
                Span::styled(
                    "y",
                    Style::default()
                        .fg(theme.success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" confirm   ", dim),
                Span::styled(
                    "n / Esc",
                    Style::default()
                        .fg(theme.error)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", dim),
            ]),
        ]),
        footer_area,
    );
}
