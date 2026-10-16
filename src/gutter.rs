//! Line numbers share the editor's font metrics and vertical origin.
use crate::theme;
use iced::{
    Color, Point, Rectangle, Renderer, Theme, alignment, mouse,
    widget::canvas::{self, Frame, Geometry, Path, Program},
};

pub struct Gutter {
    pub top: usize,
    pub offset: f32,
    pub total: usize,
    pub cursor: usize,
    pub font_size: f32,
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
        frame.fill_rectangle(
            Point::ORIGIN,
            iced::Size::new(bounds.width, 10.0),
            theme::BG_EDITOR,
        );
        frame.fill_rectangle(
            Point::new(0.0, (bounds.height - 10.0).max(0.0)),
            iced::Size::new(bounds.width, 10.0),
            theme::BG_EDITOR,
        );
        let height = self.font_size * 1.4;
        let visible = ((bounds.height - 20.0 + self.offset) / height)
            .ceil()
            .max(1.0) as usize;
        frame.with_clip(
            Rectangle::new(
                Point::new(0.0, 10.0),
                iced::Size::new(bounds.width, (bounds.height - 20.0).max(0.0)),
            ),
            |frame| {
                frame.fill_rectangle(Point::ORIGIN, bounds.size(), theme::BG_EDITOR);
                for (index, line) in (self.top..self.total).take(visible).enumerate() {
                    let y = 10.0 + index as f32 * height - self.offset;
                    if line == self.cursor {
                        frame.fill_rectangle(
                            Point::new(0.0, y),
                            iced::Size::new(bounds.width, height),
                            Color::from_rgba(0.53, 0.75, 0.82, 0.06),
                        );
                    }
                    frame.fill_text(canvas::Text {
                        content: (line + 1).to_string(),
                        position: Point::new(bounds.width - 10.0, y),
                        size: self.font_size.into(),
                        font: crate::typography::UI,
                        color: if line == self.cursor {
                            theme::TEXT_PRIMARY
                        } else {
                            theme::TEXT_MUTED
                        },
                        align_x: alignment::Horizontal::Right.into(),
                        align_y: alignment::Vertical::Top,
                        ..Default::default()
                    });
                }
            },
        );
        frame.stroke(
            &Path::line(
                Point::new(bounds.width - 1.0, 0.0),
                Point::new(bounds.width - 1.0, bounds.height),
            ),
            canvas::Stroke::default()
                .with_color(theme::BORDER_COLOR)
                .with_width(1.0),
        );
        vec![frame.into_geometry()]
    }
}
