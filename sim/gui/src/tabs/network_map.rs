#![allow(unused)]

use iced::{
	canvas::{self, event, Cache, Cursor, Event, Geometry, Path},
	mouse, Canvas, Color, Element, Length, Point, Rectangle, Vector,
};
use nalgebra::Vector2;

#[derive(Derivative)]
#[derivative(Default)]
enum Interaction {
	#[derivative(Default)]
	None,
	Selecting,
	Panning {
		translation: Vector,
		start: Point,
	},
}

#[derive(Debug, Clone)]
pub struct NetworkNode {
	position: Vector2<i64>, // Position
	size: u32,
	state: u8,
	connections: Vec<usize>, // Vector of indices representing outgoing connections
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct NetworkMap {
	pub nodes: Vec<NetworkNode>,
	node_cache: Cache,
	#[derivative(Default(value="1.0"))]
	scaling: f32,
	translation: Vector,
	interaction: Interaction,
}
#[derive(Debug, Clone)]
pub enum Message {
	// Output
	NodeClicked(usize),
	// Input
	Update,
	UpdateMap(Vec<NetworkNode>)
}
impl NetworkMap {
	const MIN_SCALING: f32 = 0.1;
	const MAX_SCALING: f32 = 2.0;
	pub fn test_conf() -> Self {
		Self {
			nodes: vec![
				NetworkNode {
					position: Vector2::new(0, 0),
					size: 30,
					state: 0,
					connections: vec![1],
				},
				NetworkNode {
					position: Vector2::new(500, 0),
					size: 30,
					state: 2,
					connections: vec![0],
				},
				NetworkNode {
					position: Vector2::new(300, 0),
					size: 30,
					state: 2,
					connections: vec![0],
				},
				NetworkNode {
					position: Vector2::new(200, -500),
					size: 40,
					state: 1,
					connections: vec![0, 1],
				},
			],
			..Default::default()
		}
	}
	pub fn update(&mut self, message: Message) {
		match message {
			Message::Update => {}
			Message::UpdateMap(nodes) => self.nodes = nodes,
			_ => unreachable!(),
		}
	}
	pub fn view<'a>(&'a mut self) -> Element<'a, Message> {
		Canvas::new(self)
			.width(Length::Fill)
			.height(Length::Fill)
			.into()
	}
}

impl<'a> canvas::Program<Message> for NetworkMap {
	fn update(
		&mut self,
		event: Event,
		bounds: Rectangle,
		cursor: Cursor,
	) -> (event::Status, Option<Message>) {
		if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
			self.interaction = Interaction::None;
		}

		let cursor_position = if let Some(position) = cursor.position_in(&bounds) {
			position
		} else {
			return (event::Status::Ignored, None);
		};

		if let Event::Mouse(mouse_event) = event {
			match mouse_event {
				mouse::Event::ButtonPressed(button) => {
					if button == mouse::Button::Left {
						self.interaction = Interaction::Panning {
							translation: self.translation,
							start: cursor_position,
						}
					}
					(event::Status::Captured, None)
				}
				mouse::Event::CursorMoved { .. } => {
					match self.interaction {
						Interaction::Panning { translation, start } => {
							self.translation = translation
								+ (cursor_position - start)
									* (1.0 / self.scaling);

							self.node_cache.clear();
							//println!("Translation: {:?}", self.translation);
						}
						_ => {},
					};

					let event_status = match self.interaction {
						Interaction::None => event::Status::Ignored,
						_ => event::Status::Captured,
					};

					(event_status, None)
				}
				mouse::Event::WheelScrolled { delta } => match delta {
					mouse::ScrollDelta::Lines { y, .. }
					| mouse::ScrollDelta::Pixels { y, .. } => {
						let old_scaling = self.scaling;

						self.scaling = (self.scaling * (1.0 + y / 30.0)).max(Self::MIN_SCALING).min(Self::MAX_SCALING);

						if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
							let factor = self.scaling - old_scaling;

							self.translation = self.translation
								- Vector::new(
									cursor_to_center.x * factor / (old_scaling * old_scaling),
									cursor_to_center.y * factor / (old_scaling * old_scaling),
								);
						}

						self.node_cache.clear();
						//println!("Scaling: {}", self.scaling);

						(event::Status::Captured, None)
					}
				},
				_ => { (event::Status::Ignored, None) },
			}
		} else {
			(event::Status::Ignored, None)
		}
		/* let cursor_position = Vector2::new(
			(self.translation.x + cursor_position.x) * self.scaling,
			(self.translation.y + cursor_position.y) * self.scaling,
		); */
		/* for node in &self.nodes {
			if (cursor_position - node.position).magnitude_squared() < size * size {

			}
		} */
	}

	fn draw(&self, bounds: Rectangle, cursor: Cursor) -> Vec<Geometry> {
		let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

		let nodes = self.node_cache.draw(bounds.size(), |frame| {
			let background = Path::rectangle(Point::ORIGIN, frame.size());
			frame.fill(&background, Color::from_rgb8(240, 240, 240));

			frame.with_save(|frame| {
				frame.translate(center);
				frame.scale(self.scaling);
				frame.translate(self.translation);
				//frame.scale(Cell::SIZE as f32);
				for node in &self.nodes {
					let point = Point::new(node.position.x as f32, node.position.y as f32);
					frame.fill(&Path::circle(point, node.size as f32), Color::BLACK);
				}
			});
		});
		vec![nodes]
	}

	fn mouse_interaction(&self, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
		match self.interaction {
			Interaction::Selecting => mouse::Interaction::Idle,
			Interaction::Panning { .. } => mouse::Interaction::Grabbing,
			Interaction::None if cursor.is_over(&bounds) => mouse::Interaction::Idle,
			_ => mouse::Interaction::default(),
		}
	}
}
