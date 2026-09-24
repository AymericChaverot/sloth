use crate::ui::state::{AppState, Tab};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Tabs,
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let mut spans = vec![
                Span::styled(
                    format!("{} ", i + 1),
                    Style::default().fg(theme.text_dimmed),
                ),
                Span::raw(tab.title()),
            ];
            if *tab == Tab::Queue && !state.selection.is_empty() {
                spans.push(Span::styled(
                    format!(" ({})", state.selection.len()),
                    Style::default().fg(theme.merged),
                ));
            }
            Line::from(spans)
        })
        .collect();

    // Click zones: Tabs draws " title " followed by a one-column divider.
    state.layout.tabs.clear();
    let mut x = area.x;
    for (tab, title) in Tab::ALL.iter().zip(&titles) {
        let width = title.width() as u16 + 2;
        state
            .layout
            .tabs
            .push((Rect::new(x, area.y, width, 1).intersection(area), *tab));
        x = x.saturating_add(width + 1);
    }

    let tabs = Tabs::new(titles)
        .select(state.tab.index())
        .style(Style::default().fg(theme.text_normal))
        .highlight_style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider(Span::styled("│", Style::default().fg(theme.border)));
    f.render_widget(tabs, area);
}
