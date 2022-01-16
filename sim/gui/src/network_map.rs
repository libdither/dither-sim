#![allow(unused)]

use std::collections::HashMap;

use iced::{Canvas, Color, Element, Length, Point, Rectangle, Vector, canvas::{self, Text, event, Cache, Cursor, Event, Geometry, Path}, keyboard, mouse};
use nalgebra::Vector2;
use petgraph::{EdgeType, Graph, graph::{EdgeIndex, NodeIndex}};

pub use petgraph::{Directed, Undirected};
use sim::FieldPosition;

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
	/// Text drawn over node
	fn text(&self) -> Option<Text>;

	/// Check if this node is being mouse over
	fn check_mouseover(&self, cursor_position: &Point) -> bool {
		let size = self.size() as f32;
		let diff = *cursor_position - self.position();
		(diff.x * diff.x + diff.y * diff.y) < size * size
	}
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
	scale: f32, // Important: this should not be zero
	translation: Vector,
	interaction: Interaction,

	global_cursor_position: Point, // Position of cursor in the global coordinate plane (i.e. before scale and translation)
	hovered_node: Option<usize>, // Current node that is being moused over
}

#[derive(Debug, Clone)]
pub enum Message {
	// Input
	TriggerNewNode(FieldPosition), // Triggers are dealt by parent in the ui model, Trigger should result in new node being added
	TriggerNewEdge(NodeIndex, NodeIndex), // They can be sent by any object, but are also produced internally
	Update, // Trigger redraw

	// Output
	NodeClicked(usize),
	EdgeClicked(usize),
	NodeDragged(usize, Vector),

	// Data output
	CanvasEvent(Event),
}
impl<N: NetworkNode, E: NetworkEdge, Ty: EdgeType> NetworkMap<N, E, Ty> {
	const MIN_SCALING: f32 = 0.1;
	const MAX_SCALING: f32 = 50.0;
	const SCALING_SPEED: f32 = 30.0;

	pub fn global_cursor_position(&self) -> &Point { &self.global_cursor_position }

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
			scale: 1.0,
			translation: Default::default(),
			interaction: Default::default(),
			global_cursor_position: Default::default(),
			hovered_node: None,
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
		let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

		let cursor_position = if let Some(position) = cursor.position_in(&bounds) {
			position
		} else {
			return (event::Status::Ignored, None);
		};
		self.global_cursor_position = Point::new(cursor_position.x * (1.0 / self.scale), cursor_position.y * (1.0 / self.scale)) - self.translation;
		

		let ret = match event {
			Event::Keyboard(_) => { (event::Status::Ignored, None) }
			Event::Mouse(mouse_event) => {
				match mouse_event {
					mouse::Event::ButtonPressed(button) => {
						// Trigger Panning
						match button {
							mouse::Button::Left => {
								if let Some(hovered) = self.hovered_node {
									(event::Status::Captured, Some(Message::NodeClicked(hovered)))
								} else {
									self.interaction = Interaction::Panning {
										translation: self.translation,
										start: cursor_position,
									};
									(event::Status::Captured, None)
								}
							}
							_ => (event::Status::Ignored, None)
						}
					}
					mouse::Event::CursorMoved { .. } => {
						match self.interaction {
							// Panning
							Interaction::Panning { translation, start } => {
								if self.scale == 0.0 { panic!("scaling should not be zero") }
								self.translation = translation + (cursor_position - start) * (1.0 / self.scale);
								self.update();
								(event::Status::Captured, None)
							}
							_ => {
								let previous_hovered = self.hovered_node;
								self.hovered_node = None;
								for node in self.nodes.node_weights() {
									if node.check_mouseover(&self.global_cursor_position) {
										let selected_id = node.unique_id();
										// sets node_selected if it is None or Some(value less than selected_id)
										if self.hovered_node < Some(selected_id) { self.hovered_node = Some(selected_id) }
									}
								}
								let status = if previous_hovered != self.hovered_node { self.update(); event::Status::Captured } else { event::Status::Ignored };
								(status, None)
							},
						}
					}
					// Set scaling
					mouse::Event::WheelScrolled { delta } => match delta {
						mouse::ScrollDelta::Lines { y, .. }
						| mouse::ScrollDelta::Pixels { y, .. } => {
							let old_scaling = self.scale;

							// Change scaling
							self.scale = (self.scale * (1.0 + y / Self::SCALING_SPEED)).max(Self::MIN_SCALING).min(Self::MAX_SCALING);

							let factor = self.scale - old_scaling;

								self.translation = self.translation
									- Vector::new(
										cursor_position.x * factor / (old_scaling * old_scaling),
										cursor_position.y * factor / (old_scaling * old_scaling),
									);

							self.update();

							(event::Status::Captured, None)
						}
					},
					_ => { (event::Status::Ignored, None) },
				}
			}
		};
		if let event::Status::Ignored = ret.0 {
			(event::Status::Ignored, Some(Message::CanvasEvent(event)))
		} else { ret }
	}

	fn draw(&self, bounds: Rectangle, cursor: Cursor) -> Vec<Geometry> {
		let center = bounds.center(); let center = Vector::new(center.x, center.y);
		let cursor_position = cursor.position();

		let mut selected: Option<usize> = None; // selected node

		let nodes = self.node_cache.draw(bounds.size(), |frame| {
			let background = Path::rectangle(Point::ORIGIN, frame.size());
			frame.fill(&background, Color::from_rgb8(240, 240, 240));

			frame.with_save(|frame| {
				//frame.translate(center);
				frame.scale(self.scale);
				frame.translate(self.translation);
				//frame.scale(Cell::SIZE as f32);
				for node in self.nodes.node_weights() {
					let point = {
						let position = node.position();
						Point::new(position.x as f32, position.y as f32)
					};
					let radius = node.size() as f32;
					
					if self.hovered_node == Some(node.unique_id()) {
						frame.fill(&Path::circle(point.clone(), radius + 5.0), Color::from_rgb8(255, 255, 0));
					}
					frame.fill(&Path::circle(point.clone(), radius), Color::BLACK);

					frame.fill_text(Text { content:
						format!("ID: {}",
						node.unique_id()),
						position: point, size: 20.0, color: Color::from_rgb8(255, 0, 0), ..Default::default()
					});
				}
			});

			frame.fill_text(Text { content:
				format!("T: ({}, {}), S: {}, FP: ({}, {})",
				self.translation.x, self.translation.y, self.scale, self.global_cursor_position.x, self.global_cursor_position.y),
				position: Point::new(0.0, 0.0), size: 20.0, ..Default::default()
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
