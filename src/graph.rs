//! This example showcases a simple native custom widget that draws a circle.
use iced::{Font, HorizontalAlignment, Vector, VerticalAlignment};
// For now, to implement a custom native widget you will need to add
// `iced_native` and `iced_wgpu` to your dependencies.
//
// Then, you simply need to define your widget type and implement the
// `iced_native::Widget` trait with the `iced_wgpu::Renderer`.
//
// Of course, you can choose to make the implementation renderer-agnostic,
// if you wish to, by creating your own `Renderer` trait, which could be
// implemented by `iced_wgpu` and other renderers.
use iced_graphics::{Backend, Defaults, Primitive, Renderer, Transformation};
use iced_native::{
	layout, mouse, Background, Color, Element, Hasher, Layout, Length, Point, Rectangle, Size,
	Widget,
};
use sim::{NetSim, Node};
use nalgebra::Point2;

pub struct Graph<'a> {
	internet: &'a NetSim<Node>,
}

impl<'a> Graph<'a> {
	pub fn new(internet: &'a NetSim<Node>) -> Self {
		Self { internet }
	}
}

impl<'a, Message, B> Widget<Message, Renderer<B>> for Graph<'a>
where
	B: Backend,
{
	fn width(&self) -> Length {
		Length::Shrink
	}

	fn height(&self) -> Length {
		Length::Shrink
	}

	fn layout(&self, _renderer: &Renderer<B>, _limits: &layout::Limits) -> layout::Node {
		layout::Node::new(Size::new(200.0, 100.0))
	}

	fn hash_layout(&self, state: &mut Hasher) {
		use std::hash::Hash;
		self.internet.router.node_map.len().hash(state);
	}

	fn draw(
		&self,
		_renderer: &mut Renderer<B>,
		_defaults: &Defaults,
		layout: Layout<'_>,
		_cursor_position: Point,
		viewport: &Rectangle,
	) -> (Primitive, mouse::Interaction) {
		let (x_range, y_range)  = &self.internet.router.field_dimensions;
		let x_scale = viewport.width / (x_range.end - x_range.start) as f32;
		let y_scale = viewport.height / (y_range.end - y_range.start) as f32;

		let primitives = self
			.internet
			.router
			.node_map
			.iter()
			.map(|(id, node)| {
				let point = node.position - Point2::new(x_range.start as f32, y_range.start as f32);
				let (x, y) = (point.x * x_scale, point.y * y_scale);
				Primitive::Group {
					primitives: vec![
						Primitive::Quad {
							bounds: Rectangle::new(Point::new(x, y), Size::new(20.0, 20.0)),
							background: Background::Color(Color::from_rgb(1.0, 0.0, 0.0)),
							border_radius: 10.0,
							border_width: 0.0,
							border_color: Color::from_rgb(0.0, 1.0, 0.0),
						},
						Primitive::Text {
							content: id.to_string(),
							/// The bounds of the text
							bounds: Rectangle::new(Point::new(x,y), Size::new(40.,20.)),
							/// The color of the text
							color: Color::from_rgb(0.0, 0.0, 1.0),
							/// The size of the text
							size: 10.0,
							/// The font of the text
							font: Font::Default,
							/// The horizontal alignment of the text
							horizontal_alignment: HorizontalAlignment::Center,
							/// The vertical alignment of the text
							vertical_alignment: VerticalAlignment::Center,
						},
					],
				}
				
			})
			.collect();
		(
			Primitive::Group { primitives },
			mouse::Interaction::default(),
		)
	}
}

impl<'a, Message, B> Into<Element<'a, Message, Renderer<B>>> for Graph<'a>
where
	B: Backend,
{
	fn into(self) -> Element<'a, Message, Renderer<B>> {
		Element::new(self)
	}
}
