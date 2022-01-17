#![allow(unused)]

use std::collections::HashMap;

use iced::{Canvas, Color, Element, Length, Point, Rectangle, Vector, canvas::{self, Cache, Cursor, Event, Frame, Geometry, Path, Stroke, Text, event}, keyboard, mouse};
use nalgebra::Vector2;
use petgraph::{EdgeType, Graph, graph::{EdgeIndex, NodeIndex}};
use either::Either;

pub use petgraph::{Directed, Undirected};
use sim::FieldPosition;

#[derive(Derivative, Debug, PartialEq)]
#[derivative(Default)]
enum Interaction {
	#[derivative(Default)]
	None, // Doing nothing else
	Hovering(usize), // Hovering over node
	PressingNode(Point, usize), // Holding down left mouse button over node
	PressingCanvas(Point), // Holding down left mouse button over canvas
	Panning { // Panning canvas
		start: Point,
	},
	Connecting { from: usize, candidate: Either<Point, usize> }, // Connecting from node to node
}
pub trait NetworkNode {
	/// Id that can be used to find the node in some other location
	fn unique_id(&self) -> usize;
	/// Position on the map of the node
	fn position(&self) -> Vector;
	/// Draw Node
	fn render(&self, frame: &mut Frame, hover: bool, selected: bool);

	/// Check if this node is being mouse over
	fn check_mouseover(&self, cursor_position: &Point) -> bool;
}
pub trait NetworkEdge {
	fn color(&self) -> Color;
	fn width(&self) -> u8;
	fn unique_connection(&self) -> (usize, usize); // Useful when adding edge to graph
}

pub struct NetworkMap<N: NetworkNode, E: NetworkEdge, Ty: EdgeType> {
	nodes: Graph<N, E, Ty>, // Node graph data structure
	unique_id_map: HashMap<usize, NodeIndex>, // Maps unique node ids to indicies into local node storage
	node_cache: Cache, // Stores geometry of last drawn update
	overlay_cache: Cache,
	translation_cache: Cache,
	scale: f32, // Current graph scaling, Important: this should not be zero
	translation: Vector, // Current Graph translation
	interaction: Interaction, // Current interaction state

	global_cursor_position: Point, // Position of cursor in the global coordinate plane (i.e. before scale and translation)
	selected_node: Option<usize>, // Current selected node
}

#[derive(Debug, Clone)]
pub enum Message {
	// Output
	NodeSelected(usize),
	EdgeClicked(usize),
	NodeDragged(usize, Vector),
	TriggerConnection(usize, usize),

	// Data output
	CanvasEvent(Event),
}
impl<N: NetworkNode, E: NetworkEdge, Ty: EdgeType> NetworkMap<N, E, Ty> {
	const MIN_SCALING: f32 = 0.1;
	const MAX_SCALING: f32 = 50.0;
	const SCALING_SPEED: f32 = 30.0;

	pub fn global_cursor_position(&self) -> &Point { &self.global_cursor_position }
	pub fn set_connecting(&mut self) {
		if let Some(selected) = self.selected_node { self.trigger_update(); self.interaction = Interaction::Connecting { from: selected, candidate: Either::Left(self.global_cursor_position) }; }
	}
	pub fn detect_hovering(&self) -> Option<usize> {
		// Detect hovering over nodes
		let mut hovering = None;
		for node in self.nodes.node_weights() {
			if node.check_mouseover(&self.global_cursor_position) {
				let selected_id = node.unique_id();
				// sets node_selected if it is None or Some(value less than selected_id)
				if hovering < Some(selected_id) { hovering = Some(selected_id) }
			}
		}
		hovering
	}

	pub fn add_node(&mut self, node: N) {
		let unique_id = node.unique_id();
		let node_index = self.nodes.add_node(node);
		self.unique_id_map.insert(unique_id, node_index);
		self.trigger_update();
	}
	/// Make sure to call NetworkMap::update()
	pub fn node_mut(&mut self, id: usize) -> Option<&mut N> { self.nodes.node_weight_mut(*self.unique_id_map.get(&id)?) }
	pub fn node(&self, id: usize) -> Option<&N> { self.nodes.node_weight(*self.unique_id_map.get(&id)?) }

	pub fn remove_node(&mut self, unique_id: usize) -> Option<()> {
		let node_index = self.unique_id_map.get(&unique_id)?;
		self.nodes.remove_node(*node_index);
		self.trigger_update();
		Some(())
	}
	pub fn trigger_update(&mut self) {
		self.overlay_cache.clear();
		self.node_cache.clear();
	}

	pub fn new() -> Self {
		Self {
			nodes: Graph::default(),
			unique_id_map: HashMap::default(),
			node_cache: Default::default(),
			translation_cache: Default::default(),
			overlay_cache: Default::default(),
			scale: 1.0,
			translation: Default::default(),
			interaction: Default::default(),
			global_cursor_position: Default::default(),
			selected_node: None,
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
		let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

		let cursor_position = if let Some(position) = cursor.position_in(&bounds) {
			position
		} else {
			return (event::Status::Ignored, None);
		};
		self.global_cursor_position = Point::new(cursor_position.x * (1.0 / self.scale), cursor_position.y * (1.0 / self.scale)) - self.translation;

		let ret: (Option<Interaction>, Option<Message>) = match event {
			Event::Keyboard(_) => { (None, None) }
			Event::Mouse(mouse_event) => {
				match mouse_event {
					mouse::Event::ButtonPressed(button) => {
						// Trigger Panning
						match button {
							mouse::Button::Left => {
								(match self.interaction {
									Interaction::Hovering(node) => Some(Interaction::PressingNode(cursor_position, node)),
									Interaction::Connecting { from: _, candidate: _ } => None,
									_ => Some(Interaction::PressingCanvas(cursor_position)),
								}, None)
							}
							_ => (None, None)
						}
					}
					mouse::Event::ButtonReleased(button) => {
						match button {
							mouse::Button::Left => {
								match self.interaction {
									Interaction::PressingNode(_, node) => {
										self.selected_node = Some(node);
										(Some(Interaction::Hovering(node)), None)
									}
									Interaction::PressingCanvas(_) => {
										(Some(Interaction::None), None)
									}
									Interaction::Connecting { from, candidate: Either::Right(to) } => {
										(Some(Interaction::None), Some(Message::TriggerConnection(from, to)))
									}
									_ => (Some(Interaction::None), None)
								}
							}
							_ => (None, None)
						}
					}
					mouse::Event::CursorMoved { position } => {
						match self.interaction {
							Interaction::PressingCanvas(start) | Interaction::PressingNode(start, _) | Interaction::Panning { start } => {
								if self.scale == 0.0 { panic!("scaling should not be zero") }
								self.translation = self.translation + (cursor_position - start) * (1.0 / self.scale);
								(Some(Interaction::Panning {
									start: cursor_position,
								}), None)
							}
							Interaction::Connecting { from, candidate } => {
								(match self.detect_hovering() {
									Some(hovering) if hovering != from => {
										Some(Interaction::Connecting { from, candidate: Either::Right(hovering) })
									},
									_ => Some(Interaction::Connecting { from, candidate: Either::Left(self.global_cursor_position) } )
								}, None)
							}
							_ => {
								let hovering = self.detect_hovering();
								(if let Some(hovering) = hovering {
									Some(Interaction::Hovering(hovering))
								} else { Some(Interaction::None) }, None)
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
							
							self.trigger_update(); // Need update here because interaction type does not change

							(None, None)
						}
					},
					_ => { (None, None) },
				}
			}
		};
		match ret {
			(None, None) => (event::Status::Ignored, Some(Message::CanvasEvent(event))),
			(Some(interaction), msg) if interaction != self.interaction => {
				use Interaction::*;
				match (&self.interaction, &interaction) {
					(Hovering(_), _) | (_, Hovering(_)) => self.node_cache.clear(),
					(Connecting { .. }, _) | (_, Connecting { .. }) => self.node_cache.clear(),
					(Panning { .. }, _) | (_, Panning { .. }) => self.node_cache.clear(),
					(PressingNode(_, _), _) => self.node_cache.clear(), // Unpress Node
					(PressingCanvas(_), _) if self.selected_node.is_some() => { self.node_cache.clear(); self.selected_node = Option::None; },
					_ => {},
				}
				self.interaction = interaction;
				self.overlay_cache.clear();
				(event::Status::Captured, msg)
			}
			(_, msg) => (event::Status::Ignored, msg),
		}
	}

	fn draw(&self, bounds: Rectangle, _: Cursor) -> Vec<Geometry> {
		let center = bounds.center(); let center = Vector::new(center.x, center.y);

		let mut selected: Option<usize> = None; // selected node

		let nodes = self.node_cache.draw(bounds.size(), |frame| {
			let background = Path::rectangle(Point::ORIGIN, frame.size());
			frame.fill(&background, Color::from_rgb8(240, 240, 240));

			// Render nodes in a scaled frame
			frame.with_save(|frame| {
				frame.scale(self.scale);
				frame.translate(self.translation);
				for node in self.nodes.node_weights() {
					let hover = if let Interaction::Hovering(hovering_node)
					 | Interaction::PressingNode(_, hovering_node)
					 | Interaction::Connecting { from: _, candidate: Either::Right(hovering_node) }
					 = self.interaction { hovering_node == node.unique_id() } else { false };
					node.render(frame, hover, self.selected_node == Some(node.unique_id()));
				}
			});
		});

		// TODO: figure out how to cache nodes and make it so that panning doesn't need redraw
		let translated_nodes = nodes;
		/* let translated_nodes = self.translation_cache.draw(bounds.size(), |frame| {
			frame.add_primitive(iced_graphics::Primitive::Translate {
				translation: self.translation,
				content: Box::new(nodes.into_primitive())
			});
		}); */
		let overlay = self.overlay_cache.draw(bounds.size(), |frame| {
			// Drawing line for connecting interaction
			frame.with_save(|frame| {
				frame.translate(self.translation);
				if let Interaction::Connecting { from, candidate } = self.interaction {
					let from = self.node(from).map(|n|Point::ORIGIN + n.position());
					if let (Some(point_from), Some(point_to)) = (from, match candidate {
						Either::Left(point) => Some(point),
						Either::Right(id) => self.node(id).map(|n|Point::ORIGIN + n.position())
					}) {
						frame.stroke(&Path::line(point_from, point_to), Stroke { width: 2.0, ..Default::default() });
					}
				}
			});
			
			frame.fill_text(Text { content:
				format!("T: ({}, {}), S: {}, FP: ({}, {}), Int: {:?}",
				self.translation.x, self.translation.y, self.scale, self.global_cursor_position.x, self.global_cursor_position.y, self.interaction),
				position: Point::new(0.0, 0.0), size: 20.0, ..Default::default()
			});
		});
		vec![translated_nodes, overlay]
	}

	/* fn mouse_interaction(&self, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
		match self.interaction {
			Interaction::Selecting => mouse::Interaction::Idle,
			Interaction::Panning { .. } => mouse::Interaction::Grabbing,
			Interaction::None if cursor.is_over(&bounds) => mouse::Interaction::Idle,
			_ => mouse::Interaction::default(),
		}
	} */
}
