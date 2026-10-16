use iced::border::Radius;
use iced::widget::{button, container, text_editor};
use iced::{Background, Border, Color, Theme};

// ----------------------------------------------------------------------------
// Nord Color Palette (Official Arctic Theme)
// ----------------------------------------------------------------------------
#[allow(dead_code)]
pub const NORD0_POLAR_DARK: Color = Color::from_rgb(0.180, 0.204, 0.251); // #2e3440 (Editor surface)
#[allow(dead_code)]
pub const NORD1_POLAR_BAR: Color = Color::from_rgb(0.231, 0.259, 0.322); // #3b4252 (Header & status bar)
#[allow(dead_code)]
pub const NORD2_POLAR_HOVER: Color = Color::from_rgb(0.263, 0.298, 0.369); // #434c5e (Hover background)
#[allow(dead_code)]
pub const NORD3_POLAR_MUTED: Color = Color::from_rgb(0.482, 0.533, 0.631); // #7b88a1 (Muted text / borders)

// Snow Storm (Text)
#[allow(dead_code)]
pub const NORD4_SNOW_BODY: Color = Color::from_rgb(0.847, 0.871, 0.914); // #d8dee9 (Body text)
#[allow(dead_code)]
pub const NORD6_SNOW_BRIGHT: Color = Color::from_rgb(0.925, 0.937, 0.957); // #eceff4 (Active text)

// Frost (Accents)
#[allow(dead_code)]
pub const NORD7_FROST_TEAL: Color = Color::from_rgb(0.561, 0.737, 0.733); // #8fbcbb (Frost teal)
#[allow(dead_code)]
pub const NORD8_FROST_ICE: Color = Color::from_rgb(0.533, 0.753, 0.816); // #88c0d0 (Frost cyan / active border)
#[allow(dead_code)]
pub const NORD9_FROST_BLUE: Color = Color::from_rgb(0.506, 0.631, 0.757); // #81a1c1 (Frost glacier blue)

// Aurora
#[allow(dead_code)]
pub const NORD12_AURORA_ORANGE: Color = Color::from_rgb(0.816, 0.529, 0.439); // #d08770 (Modified / dirty dot)
#[allow(dead_code)]
pub const NORD13_AURORA_YELLOW: Color = Color::from_rgb(0.922, 0.796, 0.545); // #ebcb8b (Warning)

// Semantic Theme Mapping
pub const BG_EDITOR: Color = NORD0_POLAR_DARK;
pub const BG_HEADER: Color = NORD1_POLAR_BAR;
pub const BG_TAB_ACTIVE: Color = NORD0_POLAR_DARK;
pub const BG_TAB_INACTIVE: Color = NORD1_POLAR_BAR;
pub const BG_TAB_HOVER: Color = NORD2_POLAR_HOVER;
pub const BG_STATUSBAR: Color = NORD1_POLAR_BAR;

pub const BORDER_COLOR: Color = Color::from_rgb(0.298, 0.337, 0.416); // #4c566a
pub const BORDER_ACTIVE: Color = NORD8_FROST_ICE;

pub const TEXT_PRIMARY: Color = NORD6_SNOW_BRIGHT;
pub const TEXT_MUTED: Color = NORD3_POLAR_MUTED;
pub const TEXT_ACCENT: Color = NORD8_FROST_ICE;
pub const DIRTY_DOT: Color = NORD12_AURORA_ORANGE;
pub const WHITESPACE_COLOR: Color = Color::from_rgba(0.482, 0.533, 0.631, 0.65); // Nord3 dimmed dot

pub fn editor_style(_theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        background: Background::Color(BG_EDITOR),
        border: Border::default(),
        placeholder: TEXT_MUTED,
        value: NORD4_SNOW_BODY,
        selection: Color::from_rgba(0.533, 0.753, 0.816, 0.28), // Frost ice selection highlight
    }
}

pub fn tab_button_style(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (bg, text_color) = if is_active {
            (BG_TAB_ACTIVE, TEXT_PRIMARY)
        } else {
            match status {
                button::Status::Hovered => (BG_TAB_HOVER, TEXT_PRIMARY),
                button::Status::Pressed => (BG_TAB_ACTIVE, TEXT_PRIMARY),
                _ => (BG_TAB_INACTIVE, TEXT_MUTED),
            }
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: if is_active {
                    BORDER_ACTIVE
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: Radius {
                    top_left: 4.0,
                    top_right: 4.0,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
            },
            ..button::Style::default()
        }
    }
}

pub fn icon_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let (bg, text_color) = match status {
        button::Status::Hovered => (Some(Background::Color(BG_TAB_HOVER)), TEXT_PRIMARY),
        button::Status::Pressed => (Some(Background::Color(BORDER_COLOR)), TEXT_PRIMARY),
        _ => (None, TEXT_MUTED),
    };

    button::Style {
        background: bg,
        text_color,
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn header_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_HEADER)),
        border: Border {
            color: BORDER_COLOR,
            width: 1.0,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

pub fn statusbar_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_STATUSBAR)),
        border: Border {
            color: BORDER_COLOR,
            width: 1.0,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

pub fn text_input_style(
    _theme: &Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    iced::widget::text_input::Style {
        background: Background::Color(NORD2_POLAR_HOVER),
        border: Border {
            color: match status {
                iced::widget::text_input::Status::Focused { .. } => BORDER_ACTIVE,
                _ => BORDER_COLOR,
            },
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: TEXT_PRIMARY,
        selection: Color::from_rgba(0.533, 0.753, 0.816, 0.3),
    }
}

pub fn picker_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_HEADER)),
        border: Border {
            color: BORDER_ACTIVE,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
        ..container::Style::default()
    }
}

pub fn picker_item_style(_theme: &Theme, status: button::Status) -> button::Style {
    let (bg, text_color) = match status {
        button::Status::Disabled => (
            None,
            Color {
                a: 0.45,
                ..TEXT_MUTED
            },
        ),
        button::Status::Hovered => (Some(Background::Color(BG_TAB_HOVER)), TEXT_PRIMARY),
        button::Status::Pressed => (Some(Background::Color(BORDER_COLOR)), TEXT_ACCENT),
        _ => (None, NORD4_SNOW_BODY),
    };

    button::Style {
        background: bg,
        text_color,
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn primary_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                NORD7_FROST_TEAL
            } else {
                NORD8_FROST_ICE
            },
        )),
        text_color: BG_EDITOR,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
