use iced::{Font, HorizontalAlignment, VerticalAlignment};

use nalgebra::Vector2;
use sim::{NetSim, Node};

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
		Length::Fill
	}

	fn height(&self) -> Length {
		Length::Fill
	}

	fn layout(&self, _renderer: &Renderer<B>, _limits: &layout::Limits) -> layout::Node {
		layout::Node::new(Size::ZERO)
	}

	fn hash_layout(&self, state: &mut Hasher) {
		use std::hash::Hash;
		self.internet.router.node_map.len().hash(state);
	}

	fn draw(
		&self,
		_renderer: &mut Renderer<B>,
		_defaults: &Defaults,
		_layout: Layout<'_>,
		_cursor_position: Point,
		viewport: &Rectangle,
	) -> (Primitive, mouse::Interaction) {
		let (x_range, y_range) = &self.internet.router.field_dimensions;
		let x_scale = viewport.width / (x_range.end - x_range.start) as f32;
		let y_scale = viewport.height / (y_range.end - y_range.start) as f32;

		let primitives = self
			.internet
			.router
			.node_map
			.iter()
			.map(|(id, node)| {
				let point = node.position - Vector2::new(x_range.start as f32, y_range.start as f32);
				let (x, y) = (point.x * x_scale, point.y * y_scale);
				Primitive::Group {
					primitives: vec![
						// Render Node as Circle
						Primitive::Quad {
							bounds: Rectangle::new(
								Point::new(x - 20.0, y - 20.0),
								Size::new(40.0, 40.0),
							),
							background: Background::Color(Color::from_rgb(0.0, 0.0, 0.0)),
							border_radius: 20.0, // Circle Radius
							border_width: 0.0, // No Border
							border_color: Color::from_rgb(0.0, 1.0, 0.0), // Circle Color
						},
						// Render Node Index
						Primitive::Text {
							content: id.to_string(), // Text
							bounds: Rectangle::new(Point::new(x, y), Size::new(60., 30.)), // Bounds of Text
							color: Color::from_rgb(1.0, 1.0, 1.0), // Color
							size: 30.0, // Size
							font: Font::Default, // Font
							horizontal_alignment: HorizontalAlignment::Center, // Horizontal Alignment
							vertical_alignment: VerticalAlignment::Center, // Vertical Alignment
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
