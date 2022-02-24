use std::net::Ipv4Addr;

use super::{Icon, Tab};
use iced::{Align, Button, Color, Column, Container, Element, Length, Point, Row, Text, Vector, button, canvas::{self, Path, Stroke, event}, keyboard};
use iced_aw::TabLabel;
use petgraph::Undirected;
use sim::{FieldPosition, NodeIdx, NodeType, WireIdx};

use crate::{gui::loaded, network_map::{self, NetworkEdge, NetworkMap, NetworkNode}};

#[derive(Clone, Debug)]
pub struct NetworkTabNode {
	id: NodeIdx,
	node_type: NodeType,
	field_position: FieldPosition,
	ip_addr: Option<Ipv4Addr>,
}
impl NetworkTabNode {
	fn new(id: NodeIdx, node_type: NodeType) -> NetworkTabNode {
		Self { id, node_type, field_position: Default::default(), ip_addr: None }
	}
}
impl NetworkNode for NetworkTabNode {
	type NodeId = NodeIdx;
	fn unique_id(&self) -> Self::NodeId { self.id }
	fn position(&self) -> Vector {
		Vector::new(self.field_position.x as f32, self.field_position.y as f32)
	}
	fn render(&self, frame: &mut canvas::Frame, hover: bool, selected: bool, scaling: f32) {
		let point = {
			Point::new(self.field_position.x as f32, self.field_position.y as f32)
		};
		let radius = 30.0;
		
		if selected {
			frame.fill(&Path::circle(point.clone(), radius + 5.0), Color::from_rgb8(255, 255, 0));
		}

		let node_color = match self.node_type { NodeType::Machine => Color::from_rgb8(39, 245, 230), NodeType::Network => Color::from_rgb8(84, 245, 39) };
		if hover { let node_color = Color::from_rgb8(200, 200, 200); }
		frame.fill(&Path::circle(point.clone(), radius), node_color);

		let label = if let Some(addr) = self.ip_addr { format!("{addr}") }
		else { format!("{}", self.id) };
		frame.fill_text(canvas::Text { content:
			label,
			position: point, color: Color::BLACK, size: radius,
			horizontal_alignment: iced::HorizontalAlignment::Center, vertical_alignment: iced::VerticalAlignment::Center,
			..Default::default()
		});
	}
	fn check_mouseover(&self, cursor_position: &Point) -> bool {
		let size = 30.0;
		let diff = *cursor_position - self.position();
		(diff.x * diff.x + diff.y * diff.y) < size * size
	}
}
#[derive(Clone, Debug)]
pub struct NetworkTabEdge {
	pub id: WireIdx,
	pub source: NodeIdx,
	pub dest: NodeIdx,
	pub latency: usize,
}
impl NetworkEdge<NetworkTabNode> for NetworkTabEdge {
	type EdgeId = WireIdx;
	fn unique_id(&self) -> Self::EdgeId { self.id }
	fn source(&self) -> NodeIdx { self.source }
	fn dest(&self) -> NodeIdx { self.dest }
	fn render(&self, frame: &mut canvas::Frame, source: & impl NetworkNode, dest: & impl NetworkNode) {
		let from = source.position();
		let to = dest.position();
		frame.stroke(&Path::line(Point::ORIGIN + from, Point::ORIGIN + to), Stroke { color: Color::from_rgb8(0, 0, 0), width: 3.0, ..Default::default() });
	}
}

#[derive(Debug, Clone)]
pub enum Message {
	AddNode(NodeIdx, sim::NodeType),
	UpdateNode(NodeIdx, sim::NodeInfo),
	UpdateMachine(NodeIdx, sim::MachineInfo),
	UpdateNetwork(NodeIdx, sim::NetworkInfo),
	UpdateConnection(WireIdx, NodeIdx, NodeIdx, bool),
	RemoveConnection(WireIdx),
	RemoveNode(NodeIdx), // Removes edges too.

	NetMapMessage(network_map::Message<NetworkTabNode, NetworkTabEdge>),
}

pub struct NetworkTab {
	map: NetworkMap<NetworkTabNode, NetworkTabEdge, Undirected>,
}

impl NetworkTab {
	pub fn new() -> Self {
		Self {
			map: NetworkMap::new(),
		}
	}
	pub fn clear(&mut self) {
		self.map = NetworkMap::new();
	}

	fn mouse_field_position(&self) -> FieldPosition {
		let cursor_pos = self.map.global_cursor_position();
		FieldPosition::new(cursor_pos.x as i32, cursor_pos.y as i32)
	}

	pub fn process(&mut self, message: Message) -> Option<loaded::Message> {
		match message {
			Message::UpdateNode(id, info) => {
				// Get node or add it if doesn't exist.
				let node = if let Some(node) = self.map.node_mut(id) { node } else {
					self.map.add_node(NetworkTabNode::new(id, info.node_type));
					self.map.node_mut(id).unwrap()
				};
				node.field_position = info.position;
				node.ip_addr = info.local_address;
				self.map.trigger_update();
			}
			Message::RemoveNode(idx) => {
				self.map.remove_node(idx);
			},
			Message::UpdateMachine(id, info) => {
				
			},
    		Message::UpdateNetwork(id, info) => {},
			
			Message::UpdateConnection(wire_idx, from, to, activation) => {
				self.map.add_edge(NetworkTabEdge { id: wire_idx, source: from, dest: to, latency: 5 });
			},
			Message::RemoveConnection(wire_idx) => {
				self.map.remove_edge(wire_idx);
			}
			Message::NetMapMessage(netmap_msg) => {
				match netmap_msg {
					network_map::Message::TriggerConnection(from, to) => {
						return Some(loaded::Message::ConnectNode(from, to));
					},
					network_map::Message::NodeDragged(node, point) => {
						return Some(loaded::Message::MoveNode(node, FieldPosition::new(point.x as i32, point.y as i32)));
					}
					network_map::Message::CanvasEvent(canvas::Event::Keyboard(keyboard_event)) => {
						match keyboard_event {
							keyboard::Event::KeyReleased { key_code, modifiers } => {
								match modifiers {
									keyboard::Modifiers { shift: false, control: false, alt: false, logo: false } => {
										match key_code {
											keyboard::KeyCode::N => {
												return Some(loaded::Message::AddNode(self.mouse_field_position(), NodeType::Network));
											}
											keyboard::KeyCode::M => {
												return Some(loaded::Message::AddNode(self.mouse_field_position(), NodeType::Machine));
											}
											keyboard::KeyCode::C => {
												self.map.set_connecting();
											}
											keyboard::KeyCode::G => {
												self.map.grab_node();
											}
											_ => {}
										}
									}
									keyboard::Modifiers { shift: false, control: true, alt: false, logo: false } => {
										match key_code {
											keyboard::KeyCode::S => {
												return Some(loaded::Message::TriggerSave);
											}
											keyboard::KeyCode::R => {
												return Some(loaded::Message::TriggerReload);
											}
											keyboard::KeyCode::P => {
												return Some(loaded::Message::DebugPrint);
											}
											_ => {},
										}
									}
									_ => {}
								}
							}
							_ => {}
						}
					}
					_ => {},
				}
			}
			_ => {}
		}
		None
	}
}

impl Tab for NetworkTab {
	type Message = Message;

	fn title(&self) -> String {
		String::from("Internet")
	}

	fn tab_label(&self) -> TabLabel {
		TabLabel::IconText(Icon::CentralizedNetwork.into(), self.title())
	}

	fn content(&mut self) -> Element<'_, Self::Message> {
		let content = Column::new().push(self.map.view().map(move |message| Message::NetMapMessage(message)));

		Container::new(content)
			.width(Length::Fill)
			.height(Length::Fill)
			.into()
	}
}


