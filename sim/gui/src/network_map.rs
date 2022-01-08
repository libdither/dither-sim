#![allow(unused)]

use std::collections::HashMap;

use iced::{
	canvas::{self, event, Cache, Cursor, Event, Geometry, Path},
	mouse, Canvas, Color, Element, Length, Point, Rectangle, Vector,
};
use nalgebra::Vector2;
use petgraph::{EdgeType, Graph, graph::{EdgeIndex, NodeIndex}};

pub use petgraph::{Directed, Undirected};

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
pub trait NetworkNode {
	fn unique_id(&self) -> usize;

	fn color(&self) -> Color;
	fn size(&self) -> u32;
	fn position(&self) -> Vector;
}
pub trait NetworkEdge {
	fn color(&self) -> Color;
	fn width(&self) -> u8;
	fn unique_connection(&self) -> (usize, usize); // Useful when adding edge to graph
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct NetworkMap<N: NetworkNode, E: NetworkEdge, Ty: EdgeType> {
	nodes: Graph<N, E, Ty>,
	unique_id_map: HashMap<usize, NodeIndex>,
	node_cache: Cache,
	#[derivative(Default(value="1.0"))]
	scaling: f32,
	translation: Vector,
	interaction: Interaction,
}
#[derive(Debug, Clone)]
pub enum Message {
	// Input
	TriggerNewNode, // Triggers are dealt by parent in the ui model, Trigger should result in new node being added
	TriggerNewEdge(NodeIndex, NodeIndex), // They can be sent by any object, but are also produced internally
	Update, // Trigger redraw

	// Output
	NodeClicked(usize),
	EdgeClicked(usize),
	NodeDragged(usize, Vector),
}
impl<N: NetworkNode, E: NetworkEdge, Ty: EdgeType> NetworkMap<N, E, Ty> {
	const MIN_SCALING: f32 = 0.1;
	const MAX_SCALING: f32 = 2.0;
	pub fn add_node(&mut self, node: N) {
		let unique_id = node.unique_id();
		let node_index = self.nodes.add_node(node);
		self.unique_id_map.insert(unique_id, node_index);
	}
	pub fn node_mut(&mut self, id: usize) -> Option<&mut N> {
		let node_idx = self.unique_id_map.get(&id)?;
		self.nodes.node_weight_mut(*node_idx)
	}
	pub fn remove_node(&mut self, unique_id: usize) -> Option<()> {
		let node_index = self.unique_id_map.get(&unique_id)?;
		self.nodes.remove_node(*node_index);
		Some(())
	}

	pub fn test_conf() -> Self {
		Self {
			nodes: Graph::default(),
			unique_id_map: HashMap::default(),
			node_cache: Default::default(),
			scaling: Default::default(),
			translation: Default::default(),
			interaction: Default::default(),
		}
	}
	pub fn update(&mut self, message: Message) -> Option<Message> {
		match message {
			Message::Update => todo!(),
			_ => Some(message),
		}
	}
	pub fn view<'a>(&'a mut self) -> Element<'a, Message> {
		Canvas::new(self)
			.width(Length::Fill)
			.height(Length::Fill)
			.into()
	}
}

impl<'a, N: NetworkNode, E: NetworkEdge, Ty: EdgeType> canvas::Program<Message> for NetworkMap<N, E, Ty> {
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
				for node in self.nodes.node_weights() {
					let position = node.position();
					let point = Point::new(position.x as f32, position.y as f32);
					frame.fill(&Path::circle(point, node.size() as f32), Color::BLACK);
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
