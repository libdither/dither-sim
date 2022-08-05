//! Generalized module for displaying and interacting with map-like data.

#![allow(unused)]

use std::{collections::HashMap, fmt};

use iced::{Color, Length, Point, Rectangle, Vector, canvas::event::Status, keyboard, mouse, pure::{Element, widget::canvas::{self, Canvas, Cache, Cursor, Event, Frame, Geometry, Path, Stroke, Text, event}}};
use nalgebra::Vector2;
use petgraph::{EdgeType, Graph, graph::{EdgeIndex, NodeIndex}};
use either::Either;

pub use petgraph::{Directed, Undirected};
use sim::FieldPosition;

/// Represents current interaction with the network map.
#[derive(Derivative, Debug, PartialEq)]
#[derivative(Default)]
pub enum Interaction {
	#[derivative(Default)]
	/// Doing Nothing
	None,
	/// Hovering over node
	Hovering(NodeIndex),
	/// Holding down left mouse button over node
	PressingNode { pos: Point, index: NodeIndex },
	/// Holding down left mouse button over canvas
	PressingCanvas { pos: Point },
	/// Panning the canvas
	Panning { pos: Point },
	/// Moving node
	MovingNode { initial_position: Point, index: NodeIndex },
	/// Connecting two nodes
	Connecting { from: NodeIndex, candidate: Either<Point, NodeIndex> },
}
#[derive(Derivative)]
#[derivative(Default)]
pub struct CanvasState {
	#[derivative(Default(value="1.0"))]
	scale: f32, // Current graph scaling, Important: this should not be zero
	translation: Vector, // Current Graph translation
	interaction: Interaction,
}

// Represents a node on the map.
pub trait NetworkNode: Sized + 'static {
	/// Unique ID used identify node in another context
	type NodeId: Sized + Clone + PartialOrd + PartialEq + Eq + std::hash::Hash;
	/// Returns this node's unique ID
	fn unique_id(&self) -> Self::NodeId;
	/// Position on the map of the node
	fn position(&self) -> Vector;
	/// Draw Node
	fn render(&self, frame: &mut Frame, hover: bool, selected: bool, scaling: f32);

	/// Check if this node is being mouse over
	fn check_mouseover(&self, cursor_position: &Point) -> bool;
}
pub trait NetworkEdge<N: NetworkNode>: Sized + 'static {
	type EdgeId: Sized + Clone + PartialOrd + PartialEq + std::hash::Hash + Eq;
	fn unique_id(&self) -> Self::EdgeId;
	fn source(&self) -> N::NodeId;
	fn dest(&self) -> N::NodeId;
	fn render(&self, frame: &mut canvas::Frame, source: & impl NetworkNode, dest: & impl NetworkNode);
	//fn unique_connection(&self) -> (usize, usize); // Useful when adding edge to graph
}

pub struct GraphWidget<N: NetworkNode, E: NetworkEdge<N>, Ty: EdgeType, M: Sized + fmt::Debug> {
	pub nodes: Graph<N, E, Ty>, // Node graph data structure
	node_id_map: HashMap<N::NodeId, NodeIndex>, // Maps unique node ids to indicies into local node storage
	edge_id_map: HashMap<E::EdgeId, EdgeIndex>,
	node_cache: Cache, // Stores geometry of last drawn update
	overlay_cache: Cache,
	translation_cache: Cache,

	pub global_cursor_position: Point, // Position of cursor in the global coordinate plane (i.e. before scale and translation)
	selected_node: Option<NodeIndex>, // Current selected node
	handle_keyboard_event: fn(keyboard::Event) -> Option<Message<N, E, M>>, // Allow for passing of function to handle events
}

#[derive(Debug, Clone)]
pub enum Message<N: NetworkNode, E: NetworkEdge<N>, M: Sized + fmt::Debug> {
	// Events
	EdgeClicked(E::EdgeId),
	NodeDragged(N::NodeId, Point),
	TriggerConnection(N::NodeId, N::NodeId),

	// Internal Events
	MouseMoved(Point),
	ClearOverlayCache,
	ClearNodeCache,
	SelectNode(Option<NodeIndex>),
	MoveCanvas(Vector),
	ScaleMoveCanvas(f32, Vector),

	// Data output
	CustomEvent(M),
}
impl<N: NetworkNode, E: NetworkEdge<N>, Ty: EdgeType, M: Sized + fmt::Debug> GraphWidget<N, E, Ty, M> {
	const MIN_SCALING: f32 = 0.1;
	const MAX_SCALING: f32 = 50.0;
	const SCALING_SPEED: f32 = 30.0;
	/// Check if there is a node that is currently being hovered over (TODO: use KD-Trees if node counts get over 100...)
	pub fn detect_hovering(&self) -> Option<NodeIndex> {
		// Detect hovering over nodes
		let mut hovering = None;
		for index in self.nodes.node_indices() {
			if self.nodes[index].check_mouseover(&self.global_cursor_position) {
				// sets node_selected if it is None or Some(value less than selected_id)
				if hovering < Some(index) { hovering = Some(index) }
			}
		}
		hovering
	}
	pub fn add_node(&mut self, node: N) {
		let unique_id = node.unique_id();
		let node_index = self.nodes.add_node(node);
		self.node_id_map.insert(unique_id, node_index);
		self.trigger_update();
	}
	pub fn add_edge(&mut self, weight: E) -> Option<()> {
		self.trigger_update();
		let edge_id = weight.unique_id();
		let edge_idx = self.nodes.add_edge(self.index(weight.source())?, self.index(weight.dest())?, weight);
		self.edge_id_map.insert(edge_id, edge_idx);
		Some(())
	}
	pub fn remove_edge(&mut self, edge_id: E::EdgeId) -> Option<E> {
		self.trigger_update();
		let edge_index = self.edge_id_map.remove(&edge_id)?;
		self.nodes.remove_edge(edge_index)
	}
	pub fn index(&self, id: N::NodeId) -> Option<NodeIndex> { self.node_id_map.get(&id).cloned() }
	/// Make sure to call NetworkMap::update()
	pub fn node_mut(&mut self, id: N::NodeId) -> Option<&mut N> { self.nodes.node_weight_mut(self.index(id)?) }
	pub fn node(&self, id: N::NodeId) -> Option<&N> { self.nodes.node_weight(self.index(id)?) }

	pub fn remove_node(&mut self, unique_id: N::NodeId) -> Option<()> {
		let node_index = self.node_id_map.get(&unique_id)?;
		self.nodes.remove_node(*node_index);
		self.trigger_update();
		Some(())
	}
	pub fn trigger_update(&mut self) {
		self.overlay_cache.clear();
		self.node_cache.clear();
	}

	pub fn new(handle_keyboard_event: fn(keyboard::Event) -> Option<Message<N, E, M>>) -> Self {
		Self {
			nodes: Graph::default(),
			node_id_map: HashMap::default(),
			edge_id_map: HashMap::default(),
			node_cache: Default::default(),
			translation_cache: Default::default(),
			overlay_cache: Default::default(),
			global_cursor_position: Default::default(),
			selected_node: None,
			handle_keyboard_event,
		}
	}
	pub fn update(&mut self, message: Message<N, E, M>) {
		match message {
			Message::MouseMoved(global_position) => {
				self.global_cursor_position = global_position;
				self.overlay_cache.clear();
			},
			Message::SelectNode(index) => {
				self.selected_node = index;
				self.node_cache.clear();
				self.overlay_cache.clear();
			},
			Message::ClearNodeCache => self.node_cache.clear(),
			Message::ClearOverlayCache => self.overlay_cache.clear(),
			_ => {},
		}
	}
	pub fn view(&self) -> Element<Message<N, E, M>> {
		Canvas::new(self)
			.width(Length::Fill)
			.height(Length::Fill)
			.into()
	}
}

impl<'a, N: NetworkNode, E: NetworkEdge<N>, Ty: EdgeType, M: Sized + fmt::Debug> canvas::Program<Message<N, E, M>> for GraphWidget<N, E, Ty, M> {
	type State = CanvasState;
	
	fn update(
		&self,
		state: &mut Self::State,
		event: Event,
		bounds: Rectangle,
		cursor: Cursor,
	) -> (Status, Option<Message<N, E, M>>) {
		let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

		let CanvasState { interaction, translation, scale } = state;

		let cursor_position = if let Some(position) = cursor.position_in(&bounds) { position }
		else { return (Status::Ignored, None); };

		let ret: (Option<Interaction>, Option<Message<N, E, M>>) = match event {
			Event::Keyboard(keyboard_event) => match keyboard_event {
				keyboard::Event::KeyReleased { key_code, modifiers } => match modifiers {
					_ if modifiers.is_empty() => {
						match key_code {
							// Trigger connecting two nodes
							keyboard::KeyCode::C => {
								if let Some(selected) = self.selected_node {
									(Some(Interaction::Connecting { from: selected, candidate: Either::Left(self.global_cursor_position) }), Some(Message::ClearOverlayCache))
								} else { (None, None) }
							}
							// Trigger grabbing a node
							keyboard::KeyCode::G => {
								if let Some(selected) = self.selected_node {
									(Some(Interaction::MovingNode { index: selected, initial_position: self.global_cursor_position }), Some(Message::ClearOverlayCache))
								} else { (None, None) }
							}
							_ => (None, (self.handle_keyboard_event)(keyboard_event))
						}
					}
					_ => (None, (self.handle_keyboard_event)(keyboard_event)),
				}
				_ => (None, (self.handle_keyboard_event)(keyboard_event))
			}
			Event::Keyboard(event) => (None, (self.handle_keyboard_event)(event)),
			Event::Mouse(mouse_event) => {
				match mouse_event {
					mouse::Event::ButtonPressed(button) => {
						// Trigger Panning
						match button {
							mouse::Button::Left => {
								match *interaction {
									Interaction::Hovering(index) => (Some(Interaction::PressingNode { pos: cursor_position, index }), None),
									Interaction::Connecting { from: _, candidate: _ } => (None, None),
									Interaction::MovingNode { index, initial_position } => {
										let node = &self.nodes[index];
										(Some(Interaction::None), Some(Message::NodeDragged(node.unique_id(),
											Point::ORIGIN + node.position() + (self.global_cursor_position.clone() - initial_position)
										)))
									}
									_ => (Some(Interaction::PressingCanvas { pos: cursor_position }), None),
								}
							}
							_ => (None, None)
						}
					}
					mouse::Event::ButtonReleased(button) => {
						match button {
							mouse::Button::Left => {
								match *interaction {
									Interaction::PressingNode { index: node, .. } => (
										Some(Interaction::Hovering(node)),
										Some(Message::SelectNode(Some(node)))
									),
									Interaction::PressingCanvas { pos } => (
										Some(Interaction::None),
										Some(Message::SelectNode(None))
									),
									Interaction::Connecting { from, candidate: Either::Right(to) } => (
										Some(Interaction::None),
										Some(Message::TriggerConnection(self.nodes[from].unique_id(), self.nodes[to].unique_id()))
									),
									Interaction::Hovering(_) => (None, None),
									_ => (Some(Interaction::None), None)
								}
							}
							_ => (None, None)
						}
					}
					mouse::Event::CursorMoved { position } => {
						let global_cursor_position = Point::new(
							cursor_position.x * (1.0 / *scale),
							cursor_position.y * (1.0 / *scale)
						) - *translation;
						let mouse_update_message = Some(Message::MouseMoved(global_cursor_position));

						match *interaction {
							Interaction::PressingCanvas { pos } | Interaction::PressingNode { pos, .. } | Interaction::Panning { pos } => {
								if *scale == 0.0 { panic!("scaling should never be zero") }
								*translation = *translation + (cursor_position - pos) * (1.0 / *scale);
								(Some(Interaction::Panning {
									pos: cursor_position,
								}), Some(Message::ClearNodeCache))
							}
							Interaction::Connecting { from, candidate } => {
								(match self.detect_hovering() {
									Some(hovering) if hovering != from => {
										Some(Interaction::Connecting { from, candidate: Either::Right(hovering) })
									},
									_ => Some(Interaction::Connecting { from, candidate: Either::Left(self.global_cursor_position) } )
								}, mouse_update_message)
							}
							Interaction::MovingNode { .. } => {
								(None, mouse_update_message)
							}
							_ => {
								let hovering = self.detect_hovering();
								(if let Some(hovering) = hovering {
									Some(Interaction::Hovering(hovering))
								} else { Some(Interaction::None) }, mouse_update_message)
							},
						}
					}
					// Set scaling
					mouse::Event::WheelScrolled { delta } => match delta {
						mouse::ScrollDelta::Lines { y, .. }
						| mouse::ScrollDelta::Pixels { y, .. } => {
							let old_scaling = *scale;

							// Change scaling
							*scale = (*scale * (1.0 + y / Self::SCALING_SPEED)).max(Self::MIN_SCALING).min(Self::MAX_SCALING);

							let factor = *scale - old_scaling;

							*translation = *translation
								- Vector::new(
									cursor_position.x * factor / (old_scaling * old_scaling),
									cursor_position.y * factor / (old_scaling * old_scaling),
								);
							(None, Some(Message::ClearNodeCache))
						}
					},
					_ => { (None, None) },
				}
			}
		};
		if let Some(interaction) = ret.0 {
			state.interaction = interaction;
		}
		if let Some(msg) = ret.1 {
			(Status::Captured, Some(msg))
		} else { (Status::Ignored, None) }
	}

	fn draw(&self, state: &Self::State, bounds: Rectangle, _: Cursor) -> Vec<Geometry> {
		let center = bounds.center(); let center = Vector::new(center.x, center.y);

		let mut selected: Option<usize> = None; // selected node

		let CanvasState { interaction, translation, scale } = state;

		let nodes = self.node_cache.draw(bounds.size(), |frame| {
			let background = Path::rectangle(Point::ORIGIN, frame.size());
			frame.fill(&background, Color::from_rgb8(240, 240, 240));

			// Render nodes in a scaled frame
			frame.with_save(|frame| {
				frame.scale(*scale);
				frame.translate(*translation);
				for edge in self.nodes.raw_edges() {
					let source = self.nodes.node_weight(edge.source()).expect("malformed graph");
					let dest = self.nodes.node_weight(edge.target()).expect("malformed graph");
					edge.weight.render(frame, source, dest);
				}

				for node_index in self.nodes.node_indices() {
					let hover = if let 
					Interaction::Hovering(hovering_node)
					 | Interaction::PressingNode { pos: _, index: hovering_node }
					 | Interaction::Connecting { from: _, candidate: Either::Right(hovering_node) }
					 = interaction { *hovering_node == node_index } else { false };
					self.nodes[node_index].render(frame, hover, self.selected_node == Some(node_index), *scale);
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
				frame.scale(*scale);
				frame.translate(*translation);
				match interaction {
					Interaction::Connecting { from, candidate } => {
						let from = self.nodes.node_weight(*from).map(|n|Point::ORIGIN + n.position());
						if let (Some(point_from), Some(point_to)) = (from, match candidate {
							Either::Left(point) => Some(*point),
							Either::Right(id) => self.nodes.node_weight(*id).map(|n|Point::ORIGIN + n.position())
						}) {
							frame.stroke(&Path::line(point_from, point_to), Stroke { width: 2.0, ..Default::default() });
						}
					}
					Interaction::MovingNode { initial_position, index } => {
						if let Some(node) = self.nodes.node_weight(*index) {
							frame.with_save(|frame|{
								frame.translate((self.global_cursor_position - *initial_position));
								node.render(frame, false, false, *scale);
							});
						}
					}
					_ => {},
				}
			});
			
			frame.fill_text(Text { content:
				format!("T: ({}, {}), S: {}, FP: ({}, {}), Int: {:?}",
				translation.x, translation.y, scale, self.global_cursor_position.x, self.global_cursor_position.y, interaction),
				position: Point::new(0.0, 0.0), size: 20.0, ..Default::default()
			});
		});
		vec![translated_nodes, overlay]
	}

	fn mouse_interaction(&self, state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
		match state.interaction {
			Interaction::Hovering(_) => mouse::Interaction::Crosshair,
			Interaction::MovingNode { .. } => mouse::Interaction::Grabbing,
			Interaction::Panning { .. } | Interaction::PressingCanvas { .. } => mouse::Interaction::Grabbing,
			Interaction::None if cursor.is_over(&bounds) => mouse::Interaction::Idle,
			_ => mouse::Interaction::default(),
		}
	}
}
