use crate::ui::state::AppState;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
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

pub fn render(f: &mut Frame, _state: &mut AppState, area: Rect) {
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
                Style::default().fg(Color::DarkGray),
            ));
        }
        header_lines.push(Line::from(spans));
    }
    let header = Paragraph::new(header_lines).alignment(ratatui::layout::Alignment::Left);
    f.render_widget(header, area);
}
