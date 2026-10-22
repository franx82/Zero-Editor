use iced::widget::{button, container, text_editor};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

// ----------------------------------------------------------------------------
// Nord Color Palette (Official Arctic Theme)
// ----------------------------------------------------------------------------
#[allow(dead_code)]
pub const NORD0_POLAR_DARK: Color = Color::from_rgb(0.180, 0.204, 0.251); // #2e3440
#[allow(dead_code)]
pub const NORD1_POLAR_BAR: Color = Color::from_rgb(0.231, 0.259, 0.322); // #3b4252
#[allow(dead_code)]
pub const NORD2_POLAR_HOVER: Color = Color::from_rgb(0.263, 0.298, 0.369); // #434c5e
#[allow(dead_code)]
pub const NORD3_POLAR_MUTED: Color = Color::from_rgb(0.298, 0.337, 0.416); // #4c566a

// Snow Storm (Text)
#[allow(dead_code)]
pub const NORD4_SNOW_BODY: Color = Color::from_rgb(0.847, 0.871, 0.914); // #d8dee9
#[allow(dead_code)]
pub const NORD6_SNOW_BRIGHT: Color = Color::from_rgb(0.925, 0.937, 0.957); // #eceff4

// Frost (Accents)
#[allow(dead_code)]
pub const NORD7_FROST_TEAL: Color = Color::from_rgb(0.561, 0.737, 0.733); // #8fbcbb
#[allow(dead_code)]
pub const NORD8_FROST_ICE: Color = Color::from_rgb(0.533, 0.753, 0.816); // #88c0d0
#[allow(dead_code)]
pub const NORD9_FROST_BLUE: Color = Color::from_rgb(0.506, 0.631, 0.757); // #81a1c1

// Aurora
#[allow(dead_code)]
pub const NORD12_AURORA_ORANGE: Color = Color::from_rgb(0.816, 0.529, 0.439); // #d08770
#[allow(dead_code)]
pub const NORD13_AURORA_YELLOW: Color = Color::from_rgb(0.922, 0.796, 0.545); // #ebcb8b

// ----------------------------------------------------------------------------
// Surfaces: the chrome sits one step below the document so the editor reads
// as the brightest plane; popups sit one step above it.
// ----------------------------------------------------------------------------
pub const BG_EDITOR: Color = NORD0_POLAR_DARK;
pub const BG_CHROME: Color = Color::from_rgb(0.161, 0.180, 0.224); // #292e39
pub const BG_ELEVATED: Color = NORD1_POLAR_BAR;
/// Translucent snow overlays, so hover and press read the same on every surface.
pub const BG_HOVER: Color = Color::from_rgba(0.847, 0.871, 0.914, 0.07);
pub const BG_PRESSED: Color = Color::from_rgba(0.847, 0.871, 0.914, 0.12);
pub const BG_CURRENT_LINE: Color = Color::from_rgba(0.847, 0.871, 0.914, 0.035);

/// Single-pixel separators between chrome and document.
pub const HAIRLINE: Color = NORD1_POLAR_BAR;
/// Outline of elevated surfaces (popups, dialogs, inputs).
pub const BORDER_ELEVATED: Color = NORD3_POLAR_MUTED;
pub const BORDER_FOCUS: Color = Color::from_rgba(0.533, 0.753, 0.816, 0.7);

pub const TEXT_PRIMARY: Color = NORD6_SNOW_BRIGHT;
pub const TEXT_BODY: Color = NORD4_SNOW_BODY;
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.639, 0.678, 0.749); // #a3adbf
pub const TEXT_MUTED: Color = Color::from_rgb(0.482, 0.533, 0.631); // #7b88a1
pub const TEXT_FAINT: Color = Color::from_rgb(0.380, 0.431, 0.533); // #616e88
pub const TEXT_ACCENT: Color = NORD8_FROST_ICE;
pub const DIRTY_DOT: Color = NORD12_AURORA_ORANGE;
pub const WHITESPACE_COLOR: Color = Color::from_rgba(0.482, 0.533, 0.631, 0.55);
pub const SELECTION: Color = Color::from_rgba(0.533, 0.753, 0.816, 0.22);

// ----------------------------------------------------------------------------
// Scales. Every surface picks from these instead of ad hoc numbers.
// ----------------------------------------------------------------------------
pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 10.0;

pub const TEXT_XS: f32 = 11.0;
pub const TEXT_SM: f32 = 12.0;
pub const TEXT_MD: f32 = 13.0;
pub const TEXT_TITLE: f32 = 15.0;

pub fn editor_style(_theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        // The pane paints the surface so the current-line band can sit beneath the text.
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: TEXT_MUTED,
        value: TEXT_BODY,
        selection: SELECTION,
    }
}

fn overlay(status: button::Status) -> Option<Background> {
    match status {
        button::Status::Hovered => Some(Background::Color(BG_HOVER)),
        button::Status::Pressed => Some(Background::Color(BG_PRESSED)),
        _ => None,
    }
}

/// Highlighted row of a picker or menu (keyboard selection).
pub fn selected_item_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.533, 0.753, 0.816, 0.16,
        ))),
        text_color: TEXT_PRIMARY,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn icon_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: overlay(status),
        text_color: if matches!(status, button::Status::Active) {
            TEXT_MUTED
        } else {
            TEXT_PRIMARY
        },
        border: Border {
            radius: RADIUS_SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Toggle button that stays lit while its option is on.
pub fn toggle_button_style(on: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let mut style = icon_button_style(theme, status);
        if on {
            style.text_color = TEXT_ACCENT;
            style.background = Some(Background::Color(Color::from_rgba(
                0.533, 0.753, 0.816, 0.14,
            )));
        }
        style
    }
}

/// Chrome strip; layouts separate it from the document with a `hairline`.
pub fn chrome_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_CHROME)),
        ..container::Style::default()
    }
}

pub fn hairline_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(HAIRLINE)),
        ..container::Style::default()
    }
}

pub fn text_input_style(
    _theme: &Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    iced::widget::text_input::Style {
        background: Background::Color(BG_EDITOR),
        border: Border {
            color: match status {
                iced::widget::text_input::Status::Focused { .. } => BORDER_FOCUS,
                _ => BORDER_ELEVATED,
            },
            width: 1.0,
            radius: RADIUS_MD.into(),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_FAINT,
        value: TEXT_PRIMARY,
        selection: SELECTION,
    }
}

pub fn picker_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_ELEVATED)),
        text_color: Some(TEXT_BODY),
        border: Border {
            color: BORDER_ELEVATED,
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 28.0,
        },
        ..container::Style::default()
    }
}

pub fn tooltip_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_ELEVATED)),
        border: Border {
            color: BORDER_ELEVATED,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    }
}

pub fn picker_item_style(_theme: &Theme, status: button::Status) -> button::Style {
    let text_color = match status {
        button::Status::Disabled => Color {
            a: 0.45,
            ..TEXT_MUTED
        },
        button::Status::Hovered | button::Status::Pressed => TEXT_PRIMARY,
        _ => TEXT_BODY,
    };

    button::Style {
        background: overlay(status),
        text_color,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn primary_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                NORD7_FROST_TEAL
            } else {
                NORD8_FROST_ICE
            },
        )),
        text_color: BG_CHROME,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
