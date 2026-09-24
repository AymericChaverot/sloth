use ratatui::style::Color;

/// Index of the theme with this name (case-insensitive), defaulting to the first one.
pub fn index_by_name(name: Option<&str>) -> usize {
    name.and_then(|name| {
        THEMES
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(name))
    })
    .unwrap_or(0)
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub error: Color,
    pub text_normal: Color,
    pub text_dimmed: Color,
    pub border: Color,
    pub border_active: Color,
    pub background: Color,
    pub merged: Color,
}

pub const THEMES: &[Theme] = &[
    // 0. Default (Classic Terminal)
    Theme {
        name: "Default",
        primary: Color::Yellow,
        secondary: Color::Cyan,
        success: Color::Green,
        error: Color::Red,
        text_normal: Color::Reset,
        text_dimmed: Color::DarkGray,
        border: Color::Reset,
        border_active: Color::Yellow,
        background: Color::Reset,
        merged: Color::LightMagenta,
    },
    // 1. Dracula
    Theme {
        name: "Dracula",
        primary: Color::Rgb(189, 147, 249),       // Purple
        secondary: Color::Rgb(139, 233, 253),     // Cyan
        success: Color::Rgb(80, 250, 123),        // Green
        error: Color::Rgb(255, 85, 85),           // Red
        text_normal: Color::Rgb(248, 248, 242),   // Foreground
        text_dimmed: Color::Rgb(98, 114, 164),    // Comment
        border: Color::Rgb(68, 71, 90),           // Current Line
        border_active: Color::Rgb(189, 147, 249), // Purple
        background: Color::Rgb(40, 42, 54),       // Background
        merged: Color::Rgb(255, 121, 198),        // Pink
    },
    // 2. Nord
    Theme {
        name: "Nord",
        primary: Color::Rgb(136, 192, 208),       // nord8 (Frost)
        secondary: Color::Rgb(129, 161, 193),     // nord9 (Frost)
        success: Color::Rgb(163, 190, 140),       // nord14 (Aurora)
        error: Color::Rgb(191, 97, 106),          // nord11 (Aurora)
        text_normal: Color::Rgb(216, 222, 233),   // nord4 (Snow Storm)
        text_dimmed: Color::Rgb(76, 86, 106),     // nord3 (Polar Night)
        border: Color::Rgb(59, 66, 82),           // nord1
        border_active: Color::Rgb(136, 192, 208), // nord8
        background: Color::Rgb(46, 52, 64),       // nord0
        merged: Color::Rgb(180, 142, 173),        // nord15 (Aurora)
    },
    // 3. Monokai
    Theme {
        name: "Monokai",
        primary: Color::Rgb(230, 219, 116),       // Yellow
        secondary: Color::Rgb(102, 217, 239),     // Light Blue
        success: Color::Rgb(166, 226, 46),        // Green
        error: Color::Rgb(249, 38, 114),          // Pink/Red
        text_normal: Color::Rgb(248, 248, 242),   // White
        text_dimmed: Color::Rgb(117, 113, 94),    // Gray
        border: Color::Rgb(39, 40, 34),           // Dark text
        border_active: Color::Rgb(230, 219, 116), // Yellow
        background: Color::Rgb(40, 40, 40),       // Dark bg
        merged: Color::Rgb(174, 129, 255),        // Purple
    },
    // 4. Catppuccin Macchiato
    Theme {
        name: "Catppuccin",
        primary: Color::Rgb(138, 173, 244),       // blue
        secondary: Color::Rgb(245, 189, 230),     // pink
        success: Color::Rgb(166, 218, 149),       // green
        error: Color::Rgb(237, 135, 150),         // red
        text_normal: Color::Rgb(202, 211, 245),   // text
        text_dimmed: Color::Rgb(91, 96, 120),     // surface2
        border: Color::Rgb(73, 77, 100),          // surface1
        border_active: Color::Rgb(138, 173, 244), // blue
        background: Color::Rgb(36, 39, 58),       // base
        merged: Color::Rgb(198, 160, 246),        // mauve
    },
];

pub fn get_theme(index: usize) -> &'static Theme {
    &THEMES[index % THEMES.len()]
}
