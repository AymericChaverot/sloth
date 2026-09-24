use crate::git::stats::format_size;
use crate::ui::state::AppState;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// HSL to RGB conversion for the rainbow title.
fn hsl_to_rgb(h: f64) -> (u8, u8, u8) {
    let s = 0.8_f64;
    let l = 0.55_f64;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h2 = h / 60.0;
    let x = c * (1.0 - ((h2 % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match h2 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

/// One line: title, scanned directory, totals, progress, update and theme.
pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let dim = Style::default().fg(theme.text_dimmed);
    let sep = || Span::styled(" · ", dim);

    let mut spans = Vec::new();
    let title = "sloth";
    for (i, ch) in title.chars().enumerate() {
        let (r, g, b) = hsl_to_rgb(i as f64 / title.len() as f64 * 300.0);
        spans.push(Span::styled(
            ch.to_string(),
            Style::default()
                .fg(Color::Rgb(r, g, b))
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled(
        format!(" v{}  ", env!("CARGO_PKG_VERSION")),
        dim,
    ));
    spans.push(Span::styled(
        state.root.display().to_string(),
        Style::default().fg(theme.secondary),
    ));

    let totals = state.totals();
    spans.push(sep());
    spans.push(Span::raw(format!("{} repos", state.repositories.len())));
    spans.push(sep());
    spans.push(Span::raw(format!(".git {}", format_size(totals.git_bytes))));
    if totals.untracked_bytes > 0 {
        spans.push(sep());
        spans.push(Span::raw(format!(
            "{} reclaimable",
            format_size(totals.untracked_bytes)
        )));
    }
    if totals.cleanable_branches > 0 {
        spans.push(sep());
        spans.push(Span::styled(
            format!("{} cleanable branches", totals.cleanable_branches),
            Style::default().fg(theme.merged),
        ));
    }

    if state.is_scanning {
        spans.push(Span::styled(
            format!("   {} scanning…", state.spinner()),
            Style::default().fg(theme.primary),
        ));
    } else if state.is_analyzing {
        let analyzed = state.repositories.iter().filter(|r| r.analyzed).count();
        spans.push(Span::styled(
            format!(
                "   {} analyzing {}/{}",
                state.spinner(),
                analyzed,
                state.repositories.len()
            ),
            Style::default().fg(theme.primary),
        ));
    }

    let mut right = Vec::new();
    if let Some(version) = &state.update_available {
        right.push(Span::styled(
            format!("↑ {version} available (u)  "),
            Style::default().fg(theme.success),
        ));
    }
    right.push(Span::styled(format!("[{}]", theme.name), dim));
    let right = Line::from(right);

    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right.width() as u16)])
            .areas(area);
    f.render_widget(Paragraph::new(Line::from(spans)), left_area);
    f.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        right_area,
    );
}
