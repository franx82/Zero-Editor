//! Tab hit testing and dragging live in one coordinate system, including overflow.
use crate::{Message, Tab, theme};
use iced::advanced::mouse;
use iced::widget::canvas::{Action, Event, Frame, Geometry, Path, Program, Stroke, Text};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, alignment};

pub const HEIGHT: f32 = 46.0;
const ACTIONS: f32 = 84.0;
const PLUS_WIDTH: f32 = 28.0;

#[derive(Debug, Clone)]
pub enum Intent {
    Select(usize),
    Close(usize),
    New,
    Split,
    Palette,
    Context(Option<String>, Point),
    Reorder { id: String, slot: usize },
    Detach(String),
}

#[derive(Debug, Default)]
pub struct State {
    offset: f32,
    pressed: Option<Press>,
    last_empty_click: Option<mouse::Click>,
    active_id: Option<String>,
}
#[derive(Debug)]
struct Press {
    id: String,
    origin: Point,
    pointer: Point,
    dragging: bool,
}

pub struct TabBar<'a> {
    pub tabs: &'a [Tab],
    pub active: usize,
    pub split: bool,
}

fn tab_width(title: &str) -> f32 {
    (title.chars().count() as f32 * 7.8 + 70.0)
        .clamp(142.0, 242.0)
        .round()
}
fn shortened(title: &str) -> String {
    if title.chars().count() <= 23 {
        title.to_owned()
    } else {
        format!("{}…", title.chars().take(22).collect::<String>())
    }
}

impl TabBar<'_> {
    fn slots(&self, offset: f32) -> Vec<Rectangle> {
        let mut x = 8.0 - offset;
        self.tabs
            .iter()
            .map(|tab| {
                let width = tab_width(&tab.title);
                let bounds = Rectangle {
                    x,
                    y: 6.0,
                    width,
                    height: HEIGHT - 12.0,
                };
                x += width + 4.0;
                bounds
            })
            .collect()
    }
    fn hit(&self, position: Point, width: f32, offset: f32) -> Option<usize> {
        if position.x >= width - ACTIONS {
            return None;
        }
        self.slots(offset).iter().position(|r| r.contains(position))
    }
    fn insertion(&self, position: Point, offset: f32) -> usize {
        self.slots(offset)
            .iter()
            .position(|r| position.x < r.center_x())
            .unwrap_or(self.tabs.len())
    }
    fn plus_rect(&self, offset: f32) -> Rectangle {
        let x = self
            .slots(offset)
            .last()
            .map(|slot| slot.x + slot.width + 6.0)
            .unwrap_or(8.0 - offset);
        Rectangle::new(Point::new(x, 9.0), Size::new(PLUS_WIDTH, HEIGHT - 18.0))
    }
    fn max_offset(&self, width: f32) -> f32 {
        (self.plus_rect(0.0).x + PLUS_WIDTH + 8.0 - (width - ACTIONS)).max(0.0)
    }
}

impl Program<Message> for TabBar<'_> {
    type State = State;
    fn update(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let emit = |intent| Some(Action::publish(Message::Tabs(intent)).and_capture());
        let old_offset = state.offset;
        state.offset = state.offset.min(self.max_offset(bounds.width));
        if let Some(active) = self.tabs.get(self.active)
            && state.active_id.as_ref() != Some(&active.id)
            && !state.pressed.as_ref().is_some_and(|p| p.dragging)
        {
            state.active_id = Some(active.id.clone());
            let slot = self.slots(state.offset)[self.active];
            if slot.x < 8.0 {
                state.offset = (state.offset + slot.x - 8.0).max(0.0);
            } else if slot.x + slot.width > bounds.width - ACTIONS - 8.0 {
                state.offset = (state.offset + slot.x + slot.width
                    - (bounds.width - ACTIONS - 8.0))
                    .min(self.max_offset(bounds.width));
            }
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                if p.x >= bounds.width - 40.0 {
                    return emit(Intent::Palette);
                }
                if p.x >= bounds.width - ACTIONS {
                    return emit(Intent::Split);
                }
                if self.plus_rect(state.offset).contains(p) {
                    return emit(Intent::New);
                }
                if let Some(index) = self.hit(p, bounds.width, state.offset) {
                    state.last_empty_click = None;
                    let rect = self.slots(state.offset)[index];
                    if p.x > rect.x + rect.width - 28.0 {
                        return emit(Intent::Close(index));
                    }
                    state.pressed = Some(Press {
                        id: self.tabs[index].id.clone(),
                        origin: p,
                        pointer: p,
                        dragging: false,
                    });
                    emit(Intent::Select(index))
                } else {
                    let click = mouse::Click::new(p, mouse::Button::Left, state.last_empty_click);
                    state.last_empty_click = Some(click);
                    if click.kind() == mouse::click::Kind::Double {
                        state.last_empty_click = None;
                        emit(Intent::New)
                    } else {
                        Some(Action::capture())
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                let index = self.hit(cursor.position_in(bounds)?, bounds.width, state.offset)?;
                emit(Intent::Close(index))
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let p = cursor.position_in(bounds)?;
                let id = self
                    .hit(p, bounds.width, state.offset)
                    .map(|i| self.tabs[i].id.clone());
                emit(Intent::Context(
                    id,
                    Point::new(bounds.x + p.x, bounds.y + p.y),
                ))
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(press) = state.pressed.as_mut() {
                    press.pointer = Point::new(position.x - bounds.x, position.y - bounds.y);
                    press.dragging |= press.origin.distance(press.pointer) > 6.0;
                    if press.dragging {
                        if press.pointer.x < 28.0 {
                            state.offset = (state.offset - 12.0).max(0.0);
                        }
                        if press.pointer.x > bounds.width - ACTIONS - 28.0 {
                            state.offset = (state.offset + 12.0).min(self.max_offset(bounds.width));
                        }
                    }
                }
                Some(Action::request_redraw())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let press = state.pressed.take()?;
                if !press.dragging {
                    return Some(Action::request_redraw());
                }
                let pointer = cursor
                    .position()
                    .map(|p| Point::new(p.x - bounds.x, p.y - bounds.y))
                    .unwrap_or(press.pointer);
                if pointer.y < -24.0 || pointer.y > HEIGHT + 24.0 {
                    emit(Intent::Detach(press.id))
                } else {
                    emit(Intent::Reorder {
                        id: press.id,
                        slot: self.insertion(pointer, state.offset),
                    })
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                let delta = match delta {
                    mouse::ScrollDelta::Lines { x, y } => (x + y) * 38.0,
                    mouse::ScrollDelta::Pixels { x, y } => x + y,
                };
                state.offset = (state.offset - delta).clamp(0.0, self.max_offset(bounds.width));
                Some(Action::request_redraw().and_capture())
            }
            _ => {
                if state.offset != old_offset {
                    Some(Action::request_redraw())
                } else {
                    None
                }
            }
        }
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(
            Point::new((bounds.width - ACTIONS).max(0.0), 0.0),
            Size::new(ACTIONS, HEIGHT),
            theme::BG_HEADER,
        );
        let pointer = cursor.position_in(bounds);
        let slots = self.slots(state.offset);
        frame.with_clip(
            Rectangle::new(
                Point::ORIGIN,
                Size::new((bounds.width - ACTIONS).max(0.0), HEIGHT),
            ),
            |f| {
                f.fill_rectangle(
                    Point::ORIGIN,
                    Size::new((bounds.width - ACTIONS).max(0.0), HEIGHT),
                    theme::BG_HEADER,
                );
                for (index, (tab, rect)) in self.tabs.iter().zip(&slots).enumerate() {
                    let active = index == self.active;
                    let hovered = pointer.is_some_and(|p| rect.contains(p));
                    let dragged = state
                        .pressed
                        .as_ref()
                        .is_some_and(|p| p.dragging && p.id == tab.id);
                    let bg = if active {
                        theme::BG_EDITOR
                    } else if hovered {
                        theme::BG_TAB_HOVER
                    } else {
                        theme::BG_HEADER
                    };
                    f.fill(
                        &Path::rounded_rectangle(rect.position(), rect.size(), 6.0.into()),
                        if dragged { Color { a: 0.45, ..bg } } else { bg },
                    );
                    if active {
                        f.fill_rectangle(
                            Point::new(rect.x + 10.0, rect.y + rect.height - 2.0),
                            Size::new(rect.width - 20.0, 2.0),
                            theme::TEXT_ACCENT,
                        );
                    }
                    if tab.is_dirty {
                        f.fill(
                            &Path::circle(Point::new(rect.x + 13.0, rect.center_y()), 3.0),
                            theme::DIRTY_DOT,
                        );
                    }
                    f.fill_text(Text {
                        content: shortened(&tab.title),
                        font: crate::typography::UI,
                        position: Point::new((rect.x + 23.0).round(), rect.y + 9.0),
                        size: 12.0.into(),
                        line_height: iced::widget::text::LineHeight::Absolute(16.0.into()),
                        shaping: iced::widget::text::Shaping::Advanced,
                        color: if active {
                            theme::TEXT_PRIMARY
                        } else {
                            theme::NORD4_SNOW_BODY
                        },
                        align_y: alignment::Vertical::Top,
                        ..Default::default()
                    });
                    if hovered || active {
                        let center = Point::new(rect.x + rect.width - 15.0, rect.center_y());
                        let stroke = Stroke::default()
                            .with_color(theme::TEXT_MUTED)
                            .with_width(1.25);
                        f.stroke(
                            &Path::line(
                                Point::new(center.x - 3.0, center.y - 3.0),
                                Point::new(center.x + 3.0, center.y + 3.0),
                            ),
                            stroke,
                        );
                        f.stroke(
                            &Path::line(
                                Point::new(center.x + 3.0, center.y - 3.0),
                                Point::new(center.x - 3.0, center.y + 3.0),
                            ),
                            stroke,
                        );
                    }
                }
                let plus = self.plus_rect(state.offset);
                let plus_hovered = pointer.is_some_and(|p| plus.contains(p));
                if plus_hovered {
                    f.fill(
                        &Path::rounded_rectangle(plus.position(), plus.size(), 6.0.into()),
                        theme::BG_TAB_HOVER,
                    );
                }
                let plus_center = plus.center();
                let plus_stroke = Stroke::default()
                    .with_color(theme::NORD4_SNOW_BODY)
                    .with_width(1.4);
                f.stroke(
                    &Path::line(
                        Point::new(plus_center.x - 4.0, plus_center.y),
                        Point::new(plus_center.x + 4.0, plus_center.y),
                    ),
                    plus_stroke,
                );
                f.stroke(
                    &Path::line(
                        Point::new(plus_center.x, plus_center.y - 4.0),
                        Point::new(plus_center.x, plus_center.y + 4.0),
                    ),
                    plus_stroke,
                );
                if let Some(press) = &state.pressed
                    && press.dragging
                {
                    let slot = self.insertion(press.pointer, state.offset);
                    let x = slots.get(slot).map(|r| r.x - 2.0).unwrap_or_else(|| {
                        slots.last().map(|r| r.x + r.width + 2.0).unwrap_or(8.0)
                    });
                    f.fill_rectangle(
                        Point::new(x, 5.0),
                        Size::new(2.0, HEIGHT - 10.0),
                        theme::TEXT_ACCENT,
                    );
                }
            },
        );
        let split_center = Point::new(bounds.width - 62.0, HEIGHT / 2.0);
        let split_box = Rectangle::new(
            Point::new(split_center.x - 8.0, split_center.y - 7.0),
            Size::new(16.0, 14.0),
        );
        let split_color = if self.split {
            theme::TEXT_ACCENT
        } else {
            theme::NORD4_SNOW_BODY
        };
        let icon_stroke = Stroke::default().with_color(split_color).with_width(1.25);
        frame.stroke(
            &Path::rounded_rectangle(split_box.position(), split_box.size(), 2.0.into()),
            icon_stroke,
        );
        frame.stroke(
            &Path::line(
                Point::new(split_center.x, split_box.y + 1.5),
                Point::new(split_center.x, split_box.y + split_box.height - 1.5),
            ),
            icon_stroke,
        );

        // The command palette is represented by Zero Editor's mark: a broken
        // zero. The gaps keep it from reading as a prohibition sign.
        let mark_center = Point::new(bounds.width - 20.0, HEIGHT / 2.0);
        let mark_stroke = Stroke::default()
            .with_color(theme::TEXT_ACCENT)
            .with_width(1.45);
        frame.stroke(&Path::circle(mark_center, 7.0), mark_stroke);
        frame.fill_rectangle(
            Point::new(mark_center.x - 2.0, mark_center.y - 8.0),
            Size::new(4.0, 3.0),
            theme::BG_HEADER,
        );
        frame.fill_rectangle(
            Point::new(mark_center.x - 2.0, mark_center.y + 5.0),
            Size::new(4.0, 3.0),
            theme::BG_HEADER,
        );
        frame.fill_rectangle(
            Point::new(0.0, HEIGHT - 1.0),
            Size::new(bounds.width, 1.0),
            theme::BORDER_COLOR,
        );
        vec![frame.into_geometry()]
    }
    fn mouse_interaction(
        &self,
        state: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.pressed.as_ref().is_some_and(|p| p.dragging) {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insertion_uses_midpoints_and_overflow_offset() {
        let tabs = vec![Tab::new_empty(0), Tab::new_empty(1)];
        let bar = TabBar {
            tabs: &tabs,
            active: 0,
            split: false,
        };
        assert_eq!(bar.insertion(Point::new(0.0, 12.0), 0.0), 0);
        assert_eq!(bar.insertion(Point::new(500.0, 12.0), 0.0), 2);
        assert_eq!(bar.hit(Point::new(20.0, 12.0), 800.0, 150.0), Some(1));
    }
    fn event(bar: &TabBar<'_>, state: &mut State, event: Event, point: Point) -> Option<Message> {
        bar.update(
            state,
            &event,
            Rectangle::new(Point::ORIGIN, Size::new(1160.0, HEIGHT)),
            mouse::Cursor::Available(point),
        )
        .and_then(|a| a.into_inner().0)
    }
    #[test]
    fn only_empty_space_double_click_creates_document() {
        let tabs = vec![Tab::new_empty(0)];
        let bar = TabBar {
            tabs: &tabs,
            active: 0,
            split: false,
        };
        let mut state = State::default();
        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        assert!(event(&bar, &mut state, press.clone(), Point::new(500.0, 23.0)).is_none());
        assert!(matches!(
            event(&bar, &mut state, press.clone(), Point::new(500.0, 23.0)),
            Some(Message::Tabs(Intent::New))
        ));
        assert!(matches!(
            event(&bar, &mut state, press, Point::new(40.0, 23.0)),
            Some(Message::Tabs(Intent::Select(0)))
        ));
    }
    #[test]
    fn plus_after_last_tab_creates_document() {
        let tabs = vec![Tab::new_empty(0), Tab::new_empty(1)];
        let bar = TabBar {
            tabs: &tabs,
            active: 0,
            split: false,
        };
        let mut state = State::default();
        let point = bar.plus_rect(0.0).center();
        assert!(matches!(
            event(
                &bar,
                &mut state,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                point
            ),
            Some(Message::Tabs(Intent::New))
        ));
    }
    #[test]
    fn drag_release_below_bar_requests_detach_with_original_identity() {
        let tabs = vec![Tab::new_empty(0)];
        let bar = TabBar {
            tabs: &tabs,
            active: 0,
            split: false,
        };
        let mut state = State::default();
        event(
            &bar,
            &mut state,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Point::new(40.0, 23.0),
        );
        let target = Point::new(40.0, 180.0);
        event(
            &bar,
            &mut state,
            Event::Mouse(mouse::Event::CursorMoved { position: target }),
            target,
        );
        let result = event(
            &bar,
            &mut state,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            target,
        );
        assert!(matches!(result,Some(Message::Tabs(Intent::Detach(ref id))) if *id==tabs[0].id));
    }
}
