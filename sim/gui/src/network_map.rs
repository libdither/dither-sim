#![allow(unused)]

use std::collections::HashMap;

use iced::{Canvas, Color, Element, Length, Point, Rectangle, Vector, canvas::{self, event, Cache, Cursor, Event, Geometry, Path}, keyboard, mouse};
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
	/// Id that can be used to find the node in some other location
	fn unique_id(&self) -> usize;

	/// Color of the node
	fn color(&self) -> Color;
	/// Size of the node
	fn size(&self) -> u32;
	/// Position on the map of the node
	fn position(&self) -> Vector;
}
pub trait NetworkEdge {
	fn color(&self) -> Color;
	fn width(&self) -> u8;
	fn unique_connection(&self) -> (usize, usize); // Useful when adding edge to graph
}

pub struct NetworkMap<N: NetworkNode, E: NetworkEdge, Ty: EdgeType> {
	nodes: Graph<N, E, Ty>,
	unique_id_map: HashMap<usize, NodeIndex>,
	node_cache: Cache,
	scaling: f32, // Important: this should not be zero
	translation: Vector,
	interaction: Interaction,
}
#[derive(Debug, Clone)]
pub enum Message {
	// Input
	TriggerNewNode(Point), // Triggers are dealt by parent in the ui model, Trigger should result in new node being added
	TriggerNewEdge(NodeIndex, NodeIndex), // They can be sent by any object, but are also produced internally
	Update, // Trigger redraw

	// Output
	NodeClicked(usize),
	EdgeClicked(usize),
	NodeDragged(usize, Vector),
}
impl<N: NetworkNode, E: NetworkEdge, Ty: EdgeType> NetworkMap<N, E, Ty> {
	const MIN_SCALING: f32 = 0.1;
	const MAX_SCALING: f32 = 5.0;
	pub fn add_node(&mut self, node: N) {
		let unique_id = node.unique_id();
		let node_index = self.nodes.add_node(node);
		self.unique_id_map.insert(unique_id, node_index);
		self.update();
	}
	/// Make sure to call NetworkMap::update()
	pub fn node_mut(&mut self, id: usize) -> Option<&mut N> {
		let node_idx = self.unique_id_map.get(&id)?;
		self.nodes.node_weight_mut(*node_idx)
	}
	pub fn remove_node(&mut self, unique_id: usize) -> Option<()> {
		let node_index = self.unique_id_map.get(&unique_id)?;
		self.nodes.remove_node(*node_index);
		self.update();
		Some(())
	}
	pub fn update(&mut self) {
		self.node_cache.clear();
	}

	pub fn new() -> Self {
		Self {
			nodes: Graph::default(),
			unique_id_map: HashMap::default(),
			node_cache: Default::default(),
			scaling: 1.0,
			translation: Default::default(),
			interaction: Default::default(),
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

		match event {
			Event::Mouse(mouse_event) => {
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
								if self.scaling == 0.0 { panic!("scaling should not be zero") }
								self.translation = translation
									+ (cursor_position - start)
										* (1.0 / self.scaling);

								self.node_cache.clear();
							}
							_ => {},
						};

						let event_status = match self.interaction {
							Interaction::None => event::Status::Ignored,
							_ => event::Status::Captured,
						};

						(event_status, None)
					}
					// Set scaling
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

							(event::Status::Captured, None)
						}
					},
					_ => { (event::Status::Ignored, None) },
				}
			}
			Event::Keyboard(keyboard_event) => {
				match keyboard_event {
					keyboard::Event::KeyReleased { key_code, modifiers } => {
						match modifiers {
							keyboard::Modifiers { shift: false, control: false, alt: false, logo: false } => {
								match key_code {
									keyboard::KeyCode::A => {
										(event::Status::Captured, Some(Message::TriggerNewNode(cursor_position)))
									}
									_ => (event::Status::Ignored, None)
								}
							}
							_ => (event::Status::Ignored, None)
						}
					}
					_ => (event::Status::Ignored, None)
				}
			}
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
