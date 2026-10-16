use crate::theme;
use iced::mouse;
use iced::widget::canvas::{Action, Event, Frame, Geometry, Path, Program};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, border};

#[derive(Debug, Clone)]
pub struct ScrollbarState {
    pub is_dragging: bool,
    pub drag_start_y: f32,
    pub drag_start_line: usize,
    pub viewport_height: f32,
    pub viewport_key: String,
    pub line_height: f32,
}

impl Default for ScrollbarState {
    fn default() -> Self {
        Self {
            is_dragging: false,
            drag_start_y: 0.0,
            drag_start_line: 0,
            viewport_height: 0.0,
            viewport_key: String::new(),
            line_height: 0.0,
        }
    }
}

pub struct Scrollbar<Message> {
    pub total_lines: usize,
    pub current_line: usize,
    pub on_scroll: Box<dyn Fn(usize) -> Message>,
    line_height: f32,
    viewport_key: String,
    on_viewport: Option<Box<dyn Fn(f32) -> Message>>,
}

impl<Message> Scrollbar<Message> {
    pub fn new(
        total_lines: usize,
        current_line: usize,
        on_scroll: impl Fn(usize) -> Message + 'static,
    ) -> Self {
        Self {
            total_lines,
            current_line,
            on_scroll: Box::new(on_scroll),
            line_height: 21.0,
            viewport_key: String::new(),
            on_viewport: None,
        }
    }

    pub fn metrics(
        mut self,
        line_height: f32,
        key: String,
        on_viewport: impl Fn(f32) -> Message + 'static,
    ) -> Self {
        self.line_height = line_height;
        self.viewport_key = key;
        self.on_viewport = Some(Box::new(on_viewport));
        self
    }

    fn visible_lines(&self, track_height: f32) -> usize {
        ((track_height - 20.0).max(0.0) / self.line_height).max(1.0) as usize
    }

    fn max_scroll(&self, track_height: f32) -> usize {
        self.total_lines
            .saturating_sub(self.visible_lines(track_height))
    }

    fn thumb_geometry(&self, track_height: f32) -> (f32, f32) {
        let visible = self.visible_lines(track_height);
        let max_scroll = self.max_scroll(track_height);
        if self.total_lines <= visible || max_scroll == 0 {
            return (0.0, 0.0);
        }

        let ratio = (visible as f32 / self.total_lines as f32).clamp(0.04, 1.0);
        let thumb_height = (track_height * ratio).clamp(24.0, (track_height - 10.0).max(24.0));
        let available_track = (track_height - thumb_height).max(0.0);

        let fraction = (self.current_line as f32 / max_scroll as f32).clamp(0.0, 1.0);
        let thumb_y = available_track * fraction;

        (thumb_y, thumb_height)
    }

    fn line_from_y(&self, y: f32, track_height: f32) -> usize {
        let visible = self.visible_lines(track_height);
        let max_scroll = self.max_scroll(track_height);
        if max_scroll == 0 {
            return 0;
        }

        let ratio = (visible as f32 / self.total_lines as f32).clamp(0.04, 1.0);
        let thumb_height = (track_height * ratio).clamp(24.0, (track_height - 10.0).max(24.0));
        let available_track = (track_height - thumb_height).max(1.0);

        let fraction = ((y - thumb_height / 2.0) / available_track).clamp(0.0, 1.0);
        (fraction * max_scroll as f32).round() as usize
    }
}

impl<Message: Clone> Program<Message> for Scrollbar<Message> {
    type State = ScrollbarState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let max_scroll = self.max_scroll(bounds.height);
        let height = (bounds.height - 20.0).max(self.line_height);
        if (state.viewport_height - height).abs() > 0.01
            || state.viewport_key != self.viewport_key
            || state.line_height != self.line_height
        {
            state.viewport_key = self.viewport_key.clone();
            state.line_height = self.line_height;
            state.viewport_height = height;
            if let Some(callback) = &self.on_viewport {
                return Some(Action::publish(callback(height)));
            }
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if max_scroll == 0 {
                    return None;
                }

                if let Some(cursor_pos) = _cursor.position_in(bounds) {
                    let (thumb_y, thumb_h) = self.thumb_geometry(bounds.height);
                    if cursor_pos.y >= thumb_y && cursor_pos.y <= thumb_y + thumb_h {
                        // Clicked inside thumb: start dragging
                        state.is_dragging = true;
                        state.drag_start_y = cursor_pos.y;
                        state.drag_start_line = self.current_line;
                        Some(Action::capture())
                    } else {
                        // Clicked on track: jump thumb to clicked position
                        let target_line = self.line_from_y(cursor_pos.y, bounds.height);
                        state.is_dragging = true;
                        state.drag_start_y = cursor_pos.y;
                        state.drag_start_line = target_line;
                        Some(Action::publish((self.on_scroll)(target_line)).and_capture())
                    }
                } else {
                    None
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.is_dragging && max_scroll > 0 {
                    let cursor_y = position.y - bounds.y;
                    let visible = self.visible_lines(bounds.height);
                    let ratio = (visible as f32 / self.total_lines as f32).clamp(0.04, 1.0);
                    let thumb_height =
                        (bounds.height * ratio).clamp(24.0, (bounds.height - 10.0).max(24.0));
                    let available_track = (bounds.height - thumb_height).max(1.0);

                    let delta_y = cursor_y - state.drag_start_y;
                    let delta_lines =
                        (delta_y / available_track * max_scroll as f32).round() as i32;
                    let new_line = (state.drag_start_line as i32 + delta_lines)
                        .clamp(0, max_scroll as i32) as usize;

                    if new_line != self.current_line {
                        return Some(Action::publish((self.on_scroll)(new_line)).and_capture());
                    }
                    Some(Action::capture())
                } else {
                    None
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.is_dragging {
                    state.is_dragging = false;
                    Some(Action::capture())
                } else {
                    None
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if max_scroll > 0 && _cursor.position_in(bounds).is_some() {
                    let lines = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => -(*y as i32),
                        mouse::ScrollDelta::Pixels { y, .. } => -(*y / 18.0).round() as i32,
                    };
                    let new_line =
                        (self.current_line as i32 + lines * 3).clamp(0, max_scroll as i32) as usize;
                    if new_line != self.current_line {
                        return Some(Action::publish((self.on_scroll)(new_line)).and_capture());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let track_width = bounds.width;
        let track_height = bounds.height;
        let is_hovered = cursor.is_over(bounds) || state.is_dragging;
        let max_scroll = self.max_scroll(track_height);

        // 1. Draw track background (always visible)
        frame.fill_rectangle(
            Point::ORIGIN,
            Size::new(track_width, track_height),
            theme::BG_TAB_INACTIVE,
        );

        // 2. Draw left border separator
        frame.fill_rectangle(
            Point::ORIGIN,
            Size::new(1.0, track_height),
            Color::from_rgba(0.298, 0.337, 0.416, 0.4), // BORDER_COLOR
        );

        // 3. Compute and draw thumb only if content overflows visible area
        if max_scroll > 0 {
            let (thumb_y, thumb_height) = self.thumb_geometry(track_height);
            let thumb_width = (track_width - 4.0).max(4.0);
            let thumb_x = (track_width - thumb_width) / 2.0;

            let thumb_color = if state.is_dragging {
                theme::NORD8_FROST_ICE
            } else if is_hovered {
                theme::NORD9_FROST_BLUE
            } else {
                Color::from_rgba(0.482, 0.533, 0.631, 0.65) // Nord3 polar muted
            };

            let thumb_path = Path::rounded_rectangle(
                Point::new(thumb_x, thumb_y),
                Size::new(thumb_width, thumb_height),
                border::Radius::from(3.0),
            );
            frame.fill(&thumb_path, thumb_color);
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.max_scroll(bounds.height) == 0 {
            return mouse::Interaction::default();
        }

        if state.is_dragging {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrollbar_short_content() {
        let scrollbar = Scrollbar::new(10, 0, |line| line);
        let track_height = 600.0; // ~28 visible lines
        assert_eq!(scrollbar.max_scroll(track_height), 0);
        let (thumb_y, thumb_h) = scrollbar.thumb_geometry(track_height);
        assert_eq!(thumb_y, 0.0);
        assert_eq!(thumb_h, 0.0);
    }

    #[test]
    fn test_scrollbar_long_content() {
        let scrollbar = Scrollbar::new(200, 0, |line| line);
        let track_height = 600.0;
        let max_scroll = scrollbar.max_scroll(track_height);
        assert!(max_scroll > 150);

        let (thumb_y, thumb_h) = scrollbar.thumb_geometry(track_height);
        assert_eq!(thumb_y, 0.0);
        assert!(thumb_h >= 24.0);

        // At end of document
        let scrollbar_end = Scrollbar::new(200, max_scroll, |line| line);
        let (thumb_y_end, thumb_h_end) = scrollbar_end.thumb_geometry(track_height);
        assert_eq!(thumb_h, thumb_h_end);
        assert!((thumb_y_end + thumb_h_end - track_height).abs() < 1.0);
    }

    #[test]
    fn test_scrollbar_line_from_y() {
        let scrollbar = Scrollbar::new(100, 0, |line| line);
        let track_height = 500.0;
        let line_top = scrollbar.line_from_y(0.0, track_height);
        assert_eq!(line_top, 0);

        let line_bottom = scrollbar.line_from_y(track_height, track_height);
        let max_scroll = scrollbar.max_scroll(track_height);
        assert_eq!(line_bottom, max_scroll);
    }
}
