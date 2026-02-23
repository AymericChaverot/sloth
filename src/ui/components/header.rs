use crate::ui::state::AppState;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

/// HSL to RGB conversion for rainbow gradient
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

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let version = env!("CARGO_PKG_VERSION");
    let art_lines = [
        "  ▄▄▄▄▄  ▄▄                ",
        " ██▀▀▀▀█▄ ██       █▄ █▄   ",
        " ▀██▄  ▄▀ ██      ▄██▄██   ",
        "   ▀██▄▄  ██ ▄███▄ ██ ████▄",
        " ▄   ▀██▄ ██ ██ ██ ██ ██ ██",
        " ▀██████▀▄██▄▀███▀▄██▄██ ██",
    ];
    let max_diag = (art_lines.len() + 28) as f64;

    let mut header_lines: Vec<Line> = Vec::new();
    header_lines.push(Line::from("")); // top padding
    for (row, art) in art_lines.iter().enumerate() {
        let mut spans: Vec<Span> = Vec::new();
        for (col, ch) in art.chars().enumerate() {
            let diag = (row + col) as f64;
            let hue = (diag / max_diag) * 300.0;
            let (r, g, b) = hsl_to_rgb(hue);
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(Color::Rgb(r, g, b)),
            ));
        }
        if row == art_lines.len() - 1 {
            spans.push(Span::styled(
                format!(" v{} - The git repository cleaner tool", version),
                Style::default().fg(theme.text_dimmed),
            ));
        }
        header_lines.push(Line::from(spans));
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)].as_ref())
        .split(area);

    let header = Paragraph::new(header_lines).alignment(Alignment::Left);
    f.render_widget(header, chunks[0]);

    let theme_label = Paragraph::new(format!("Theme: {}", theme.name))
        .style(Style::default().fg(theme.primary))
        .alignment(Alignment::Right)
        .block(Block::default());

    // We want the theme label vertically aligned at the bottom (or same level as the version line).
    // The version line is at art_lines.len(). We can just lay it out.
    let theme_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(art_lines.len() as u16),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(chunks[1]);

    f.render_widget(theme_label, theme_layout[1]);
}
