use crate::ui::state::{AppState, Tab};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Tabs,
};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
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
