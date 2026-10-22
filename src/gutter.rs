//! Line numbers share the editor's font metrics and vertical origin.
use crate::theme;
use iced::{
    Point, Rectangle, Renderer, Theme, alignment, mouse,
    widget::canvas::{self, Frame, Geometry, Program},
};

pub struct Gutter {
    pub top: usize,
    pub offset: f32,
    pub total: usize,
    pub cursor: usize,
    pub font_size: f32,
}
impl Gutter {
    fn line_height(&self) -> f32 {
        self.font_size * 1.4
    }
}

impl<Message> Program<Message> for Gutter {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), theme::BG_EDITOR);
        let height = self.line_height();
        let visible = ((bounds.height - 2.0 * PADDING + self.offset) / height)
            .ceil()
            .max(1.0) as usize;
        frame.with_clip(
            Rectangle::new(
                Point::new(0.0, PADDING),
                iced::Size::new(bounds.width, (bounds.height - 2.0 * PADDING).max(0.0)),
            ),
            |frame| {
                for (index, line) in (self.top..self.total).take(visible).enumerate() {
                    let y = PADDING + index as f32 * height - self.offset;
                    let current = line == self.cursor;
                    if current {
                        frame.fill_rectangle(
                            Point::new(0.0, y),
                            iced::Size::new(bounds.width, height),
                            theme::BG_CURRENT_LINE,
                        );
                    }
                    frame.fill_text(canvas::Text {
                        content: (line + 1).to_string(),
                        position: Point::new(bounds.width - 12.0, y + height / 2.0),
                        size: (self.font_size * 0.9).into(),
                        font: crate::typography::MONO,
                        color: if current {
                            theme::TEXT_SECONDARY
                        } else {
                            theme::TEXT_FAINT
                        },
                        align_x: alignment::Horizontal::Right.into(),
                        align_y: alignment::Vertical::Center,
                        ..Default::default()
                    });
                }
            },
        );
        vec![frame.into_geometry()]
    }
}

/// Editor padding above the first and below the last line.
const PADDING: f32 = 10.0;

/// Current-line band painted beneath the text, aligned with the gutter rows.
pub struct LineBand {
    pub top: usize,
    pub offset: f32,
    pub cursor: usize,
    pub font_size: f32,
}
impl<Message> Program<Message> for LineBand {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let height = self.font_size * 1.4;
        if self.cursor >= self.top {
            let y = PADDING + (self.cursor - self.top) as f32 * height - self.offset;
            frame.with_clip(
                Rectangle::new(
                    Point::new(0.0, PADDING),
                    iced::Size::new(bounds.width, (bounds.height - 2.0 * PADDING).max(0.0)),
                ),
                |frame| {
                    frame.fill_rectangle(
                        Point::new(0.0, y),
                        iced::Size::new(bounds.width, height),
                        theme::BG_CURRENT_LINE,
                    );
                },
            );
        }
        vec![frame.into_geometry()]
    }
}
