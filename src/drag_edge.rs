//! Keeps a selection drag going once the pointer leaves the editor sideways:
//! iced stops reporting drags outside the widget, so this republishes the
//! pointer every frame until the button is released.

use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector, window};

/// Presses this close to the bottom edge belong to the horizontal scrollbar.
const SCROLLBAR_HEIGHT: f32 = 10.0;

pub struct DragEdge<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_drag: Box<dyn Fn(Point, Size) -> Message + 'a>,
}

/// `on_drag` receives the pointer relative to the content bounds, and their size.
pub fn drag_edge<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_drag: impl Fn(Point, Size) -> Message + 'a,
) -> DragEdge<'a, Message, Theme, Renderer> {
    DragEdge {
        content: content.into(),
        on_drag: Box::new(on_drag),
    }
}

#[derive(Default)]
struct State {
    dragging: bool,
    pointer: Option<Point>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DragEdge<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.dragging = cursor
                    .position_over(bounds)
                    .is_some_and(|p| p.y < bounds.y + bounds.height - SCROLLBAR_HEIGHT);
                state.pointer = None;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                state.pointer = None;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
                let outside = position.x < bounds.x || position.x > bounds.x + bounds.width;
                state.pointer = outside.then_some(*position);
                if outside {
                    shell.request_redraw();
                }
            }
            Event::Window(window::Event::RedrawRequested(_)) if state.dragging => {
                if let Some(position) = state.pointer {
                    shell.publish((self.on_drag)(
                        position - Vector::new(bounds.x, bounds.y),
                        bounds.size(),
                    ));
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer> From<DragEdge<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(widget: DragEdge<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}
