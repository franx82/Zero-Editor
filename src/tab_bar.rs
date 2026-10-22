//! Tab hit testing and dragging live in one coordinate system, including overflow.
use crate::{Message, Tab, theme};
use iced::advanced::mouse;
use iced::widget::canvas::path::Arc;
use iced::widget::canvas::{Action, Event, Frame, Geometry, Path, Program, Stroke, Text};
use iced::{Color, Point, Radians, Rectangle, Renderer, Size, Theme, alignment};

/// The bar doubles as the macOS titlebar, so it is the only chrome above the document.
pub const HEIGHT: f32 = 38.0;
const ACTIONS: f32 = 84.0;
const PLUS_WIDTH: f32 = 26.0;
/// Room for the native traffic lights, which sit inside the bar on macOS.
#[cfg(target_os = "macos")]
const START: f32 = 78.0;
#[cfg(not(target_os = "macos"))]
const START: f32 = 8.0;

#[derive(Debug, Clone)]
pub enum Intent {
    Select(usize),
    Close(usize),
    New,
    Split,
    Palette,
    Context(Option<String>, Point),
    Reorder {
        id: String,
        slot: usize,
    },
    /// Released outside the bar; the point is relative to the bar's origin.
    Detach(String, Point),
    DragWindow,
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
    (shortened(title).chars().count() as f32 * 6.9 + 52.0)
        .clamp(112.0, 224.0)
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
        let mut x = START - offset;
        self.tabs
            .iter()
            .map(|tab| {
                let width = tab_width(&tab.title);
                let bounds = Rectangle {
                    x,
                    y: 0.0,
                    width,
                    height: HEIGHT,
                };
                x += width;
                bounds
            })
            .collect()
    }
    fn hit(&self, position: Point, width: f32, offset: f32) -> Option<usize> {
        if position.x < START || position.x >= width - ACTIONS {
            return None;
        }
        self.slots(offset).iter().position(|r| r.contains(position))
    }
    /// Where a dragged tab is drawn and which position it would take among the
    /// other tabs. It follows the pointer horizontally, kept inside the strip.
    fn drag_target(
        &self,
        press: &Press,
        offset: f32,
        width: f32,
    ) -> Option<(usize, Rectangle, usize)> {
        let dragged = self.tabs.iter().position(|t| t.id == press.id)?;
        let slots = self.slots(offset);
        let home = slots[dragged];
        let max_x = (width - ACTIONS - home.width).max(START);
        let x = (home.x + press.pointer.x - press.origin.x).clamp(START, max_x);
        let rect = Rectangle { x, ..home };
        let position = slots
            .iter()
            .enumerate()
            .filter(|(index, slot)| *index != dragged && slot.center_x() < rect.center_x())
            .count();
        Some((dragged, rect, position))
    }
    /// Tab rectangles as drawn: while dragging inside the bar, the other tabs
    /// close ranks and open a gap where the dragged tab would land.
    fn layout(&self, state: &State, width: f32) -> (Vec<Rectangle>, Option<usize>) {
        let mut slots = self.slots(state.offset);
        let Some(press) = state.pressed.as_ref().filter(|p| p.dragging) else {
            return (slots, None);
        };
        let Some((dragged, rect, position)) = self.drag_target(press, state.offset, width) else {
            return (slots, None);
        };
        let gap = (!detaching(press.pointer, width)).then_some(position);
        let mut x = START - state.offset;
        let mut placed = 0;
        for (index, slot) in slots.iter_mut().enumerate() {
            if index == dragged {
                continue;
            }
            if gap == Some(placed) {
                x += rect.width;
            }
            slot.x = x;
            x += slot.width;
            placed += 1;
        }
        slots[dragged] = rect;
        (slots, Some(dragged))
    }
    fn plus_rect(&self, offset: f32) -> Rectangle {
        let x = self
            .slots(offset)
            .last()
            .map(|slot| slot.x + slot.width + 6.0)
            .unwrap_or(START - offset);
        Rectangle::new(
            Point::new(x, (HEIGHT - PLUS_WIDTH) / 2.0),
            Size::new(PLUS_WIDTH, PLUS_WIDTH),
        )
    }
    fn max_offset(&self, width: f32) -> f32 {
        (self.plus_rect(0.0).x + PLUS_WIDTH + 8.0 - (width - ACTIONS)).max(0.0)
    }
}

/// Releasing this far above or below the bar, or beyond the window's sides,
/// takes the tab out of the window.
fn detaching(pointer: Point, width: f32) -> bool {
    pointer.y < -24.0 || pointer.y > HEIGHT + 24.0 || pointer.x < 0.0 || pointer.x > width
}

/// Position a tab dropped at `x` takes in a bar showing `tabs` unscrolled.
pub fn drop_index(tabs: &[Tab], x: f32) -> usize {
    let bar = TabBar {
        tabs,
        active: 0,
        split: false,
    };
    bar.slots(0.0)
        .iter()
        .position(|slot| x < slot.center_x())
        .unwrap_or(tabs.len())
}

fn plus_icon(frame: &mut Frame, center: Point, color: Color) {
    let stroke = Stroke::default().with_color(color).with_width(1.3);
    frame.stroke(
        &Path::line(
            Point::new(center.x - 4.5, center.y),
            Point::new(center.x + 4.5, center.y),
        ),
        stroke,
    );
    frame.stroke(
        &Path::line(
            Point::new(center.x, center.y - 4.5),
            Point::new(center.x, center.y + 4.5),
        ),
        stroke,
    );
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
            if slot.x < START {
                state.offset = (state.offset + slot.x - START).max(0.0);
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
                        emit(Intent::DragWindow)
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
                        if press.pointer.x < START + 20.0 {
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
                if detaching(pointer, bounds.width) {
                    return emit(Intent::Detach(press.id, pointer));
                }
                let press = Press { pointer, ..press };
                let (from, _, to) = self.drag_target(&press, state.offset, bounds.width)?;
                // `Reorder` counts slots before the tab is removed from its old place.
                emit(Intent::Reorder {
                    id: press.id,
                    slot: if to >= from { to + 1 } else { to },
                })
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
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), theme::BG_CHROME);
        let pointer = cursor.position_in(bounds);
        let (slots, lifted) = self.layout(state, bounds.width);
        let strip_end = (bounds.width - ACTIONS).max(START);
        let mut active_span = None;
        frame.with_clip(
            Rectangle::new(Point::new(START, 0.0), Size::new(strip_end - START, HEIGHT)),
            |f| {
                // The lifted tab is drawn last so it slides over its neighbours.
                let order = (0..self.tabs.len())
                    .filter(|i| Some(*i) != lifted)
                    .chain(lifted);
                for index in order {
                    let (tab, rect) = (&self.tabs[index], &slots[index]);
                    let active = index == self.active;
                    let dragged = lifted == Some(index);
                    let hovered = dragged
                        || lifted.is_none()
                            && pointer.is_some_and(|p| p.x >= START && rect.contains(p));
                    let outside = dragged
                        && state
                            .pressed
                            .as_ref()
                            .is_some_and(|p| detaching(p.pointer, bounds.width));
                    let fade = |c: Color| {
                        if outside {
                            Color { a: c.a * 0.45, ..c }
                        } else {
                            c
                        }
                    };
                    if active {
                        f.fill_rectangle(rect.position(), rect.size(), fade(theme::BG_EDITOR));
                        active_span = Some((rect.x, rect.x + rect.width));
                    } else if dragged {
                        f.fill_rectangle(rect.position(), rect.size(), fade(theme::BG_ELEVATED));
                    } else if hovered {
                        f.fill_rectangle(rect.position(), rect.size(), theme::BG_HOVER);
                    }
                    if dragged || (rect.x - (START - state.offset)).abs() < 0.5 {
                        f.fill_rectangle(
                            Point::new(rect.x, 0.0),
                            Size::new(1.0, HEIGHT),
                            theme::HAIRLINE,
                        );
                    }
                    f.fill_rectangle(
                        Point::new(rect.x + rect.width - 1.0, 0.0),
                        Size::new(1.0, HEIGHT),
                        theme::HAIRLINE,
                    );
                    f.fill_text(Text {
                        content: shortened(&tab.title),
                        font: crate::typography::UI,
                        position: Point::new((rect.x + 14.0).round(), rect.center_y()),
                        size: theme::TEXT_SM.into(),
                        line_height: iced::widget::text::LineHeight::Absolute(16.0.into()),
                        shaping: iced::widget::text::Shaping::Advanced,
                        color: fade(if active {
                            theme::TEXT_PRIMARY
                        } else if hovered {
                            theme::TEXT_SECONDARY
                        } else {
                            theme::TEXT_MUTED
                        }),
                        align_y: alignment::Vertical::Center,
                        ..Default::default()
                    });
                    // The close control doubles as the unsaved marker: a dot until
                    // the tab is hovered, then a cross.
                    let center = Point::new(rect.x + rect.width - 17.0, rect.center_y());
                    if tab.is_dirty && !hovered {
                        f.fill(&Path::circle(center, 3.5), fade(theme::DIRTY_DOT));
                    } else if hovered || active {
                        let target = Rectangle::new(
                            Point::new(center.x - 9.0, center.y - 9.0),
                            Size::new(18.0, 18.0),
                        );
                        if pointer.is_some_and(|p| target.contains(p)) {
                            f.fill(
                                &Path::rounded_rectangle(
                                    target.position(),
                                    target.size(),
                                    theme::RADIUS_SM.into(),
                                ),
                                theme::BG_HOVER,
                            );
                        }
                        let stroke = Stroke::default()
                            .with_color(if hovered {
                                theme::TEXT_SECONDARY
                            } else {
                                theme::TEXT_MUTED
                            })
                            .with_width(1.2);
                        f.stroke(
                            &Path::line(
                                Point::new(center.x - 3.5, center.y - 3.5),
                                Point::new(center.x + 3.5, center.y + 3.5),
                            ),
                            stroke,
                        );
                        f.stroke(
                            &Path::line(
                                Point::new(center.x + 3.5, center.y - 3.5),
                                Point::new(center.x - 3.5, center.y + 3.5),
                            ),
                            stroke,
                        );
                    }
                }
                let plus = self.plus_rect(state.offset);
                if pointer.is_some_and(|p| plus.contains(p)) {
                    f.fill(
                        &Path::rounded_rectangle(
                            plus.position(),
                            plus.size(),
                            theme::RADIUS_MD.into(),
                        ),
                        theme::BG_HOVER,
                    );
                }
                plus_icon(f, plus.center(), theme::TEXT_MUTED);
            },
        );

        // The document's top edge, broken under the active tab so it flows into the editor.
        let (gap_start, gap_end) = active_span
            .map(|(a, b)| (a.max(START) + 1.0, b.min(strip_end) - 1.0))
            .unwrap_or((0.0, 0.0));
        frame.fill_rectangle(
            Point::new(0.0, HEIGHT - 1.0),
            Size::new(gap_start.max(0.0), 1.0),
            theme::HAIRLINE,
        );
        frame.fill_rectangle(
            Point::new(gap_end.max(gap_start), HEIGHT - 1.0),
            Size::new((bounds.width - gap_end.max(gap_start)).max(0.0), 1.0),
            theme::HAIRLINE,
        );

        let action = |center: Point| {
            Rectangle::new(
                Point::new(center.x - 14.0, center.y - 13.0),
                Size::new(28.0, 26.0),
            )
        };
        let split_center = Point::new(bounds.width - 62.0, HEIGHT / 2.0);
        let mark_center = Point::new(bounds.width - 24.0, HEIGHT / 2.0);
        for center in [split_center, mark_center] {
            let target = action(center);
            if pointer.is_some_and(|p| target.contains(p)) {
                frame.fill(
                    &Path::rounded_rectangle(
                        target.position(),
                        target.size(),
                        theme::RADIUS_MD.into(),
                    ),
                    theme::BG_HOVER,
                );
            }
        }

        let split_box = Rectangle::new(
            Point::new(split_center.x - 7.5, split_center.y - 6.5),
            Size::new(15.0, 13.0),
        );
        if self.split {
            frame.fill(
                &Path::rounded_rectangle(
                    split_box.position(),
                    Size::new(split_box.width / 2.0, split_box.height),
                    2.0.into(),
                ),
                Color {
                    a: 0.25,
                    ..theme::TEXT_ACCENT
                },
            );
        }
        let icon_stroke = Stroke::default()
            .with_color(if self.split {
                theme::TEXT_ACCENT
            } else {
                theme::TEXT_MUTED
            })
            .with_width(1.2);
        frame.stroke(
            &Path::rounded_rectangle(split_box.position(), split_box.size(), 2.5.into()),
            icon_stroke,
        );
        frame.stroke(
            &Path::line(
                Point::new(split_center.x, split_box.y + 1.0),
                Point::new(split_center.x, split_box.y + split_box.height - 1.0),
            ),
            icon_stroke,
        );

        // The command palette is represented by Zero Editor's mark: a broken
        // zero. The gaps keep it from reading as a prohibition sign.
        let mark_stroke = Stroke::default()
            .with_color(theme::TEXT_ACCENT)
            .with_width(1.4);
        let gap = 0.3;
        for start in [-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2] {
            frame.stroke(
                &Path::new(|p| {
                    p.arc(Arc {
                        center: mark_center,
                        radius: 6.5,
                        start_angle: Radians(start + gap),
                        end_angle: Radians(start + std::f32::consts::PI - gap),
                    })
                }),
                mark_stroke,
            );
        }
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
    fn hit_testing_follows_overflow_offset() {
        let tabs = vec![Tab::new_empty(0), Tab::new_empty(1)];
        let bar = TabBar {
            tabs: &tabs,
            active: 0,
            split: false,
        };
        let offset = bar.slots(0.0)[0].width;
        assert_eq!(
            bar.hit(Point::new(START + 20.0, 12.0), 800.0, offset),
            Some(1)
        );
        assert_eq!(bar.hit(Point::new(START - 20.0, 12.0), 800.0, offset), None);
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
        assert!(matches!(
            event(&bar, &mut state, press.clone(), Point::new(500.0, 23.0)),
            Some(Message::Tabs(Intent::DragWindow))
        ));
        assert!(matches!(
            event(&bar, &mut state, press.clone(), Point::new(500.0, 23.0)),
            Some(Message::Tabs(Intent::New))
        ));
        assert!(matches!(
            event(&bar, &mut state, press, Point::new(START + 40.0, 23.0)),
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
    fn dragging_past_a_neighbour_centre_moves_the_tab_after_it() {
        let tabs = vec![Tab::new_empty(0), Tab::new_empty(1), Tab::new_empty(2)];
        let bar = TabBar {
            tabs: &tabs,
            active: 0,
            split: false,
        };
        let mut state = State::default();
        let slots = bar.slots(0.0);
        let start = slots[0].center();
        event(
            &bar,
            &mut state,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            start,
        );
        // Carry the first tab's centre just past the second tab's centre.
        let target = Point::new(slots[1].center_x() + 4.0, start.y);
        event(
            &bar,
            &mut state,
            Event::Mouse(mouse::Event::CursorMoved { position: target }),
            target,
        );
        let (layout, lifted) = bar.layout(&state, 1160.0);
        assert_eq!(lifted, Some(0));
        assert_eq!(
            layout[1].x, START,
            "the neighbour slides into the vacated slot"
        );
        let result = event(
            &bar,
            &mut state,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            target,
        );
        assert!(matches!(
            result,
            Some(Message::Tabs(Intent::Reorder { slot: 2, .. }))
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
            Point::new(START + 40.0, 23.0),
        );
        let target = Point::new(START + 40.0, 180.0);
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
        assert!(matches!(result,Some(Message::Tabs(Intent::Detach(ref id, _))) if *id==tabs[0].id));
    }
}
