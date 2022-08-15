use std::net::Ipv4Addr;

use super::{Icon, Tab};
use iced::{Color, Length, Point, Row, Vector, alignment::{Horizontal, Vertical}, button, pure::{Element, column, container, widget::canvas::{self, Path, Stroke, event}}, keyboard};
use iced_aw::pure::TabLabel;
use petgraph::Undirected;
use sim::{FieldPosition, NodeIdx, NodeType, WireIdx};

use crate::{gui::loaded, graph_widget::{self, NetworkEdge, GraphWidget, NetworkNode}};

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

		let fp_str = format!("({}, {})", self.field_position.x, self.field_position.y);

		let label = if let Some(addr) = self.ip_addr { format!("{addr}\n{}", fp_str) }
		else { format!("{}\n{}", self.id, fp_str) };
		frame.fill_text(canvas::Text { content:
			label,
			position: point, color: Color::BLACK, size: radius / 2.0,
			horizontal_alignment: Horizontal::Center, vertical_alignment: Vertical::Center,
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
pub enum NetworkMapEvent {
	AddMachine,
	AddNetwork,
	TriggerSave,
	TriggerReload,
	TriggerDebugPrint,
}
type NetworkMapMessage = graph_widget::Message<NetworkTabNode, NetworkTabEdge, NetworkMapEvent>;
type NetworkMap = graph_widget::GraphWidget<NetworkTabNode, NetworkTabEdge, Undirected, NetworkMapEvent>;

#[derive(Debug, Clone)]
pub enum Message {
	AddNode(NodeIdx, sim::NodeType),
	UpdateNode(NodeIdx, sim::NodeInfo),
	UpdateMachine(NodeIdx, sim::MachineInfo),
	UpdateNetwork(NodeIdx, sim::NetworkInfo),
	UpdateConnection(WireIdx, NodeIdx, NodeIdx, bool),
	RemoveConnection(WireIdx),
	RemoveNode(NodeIdx), // Removes edges too.

	MapMessage(NetworkMapMessage),
}

pub struct NetworkTab {
	map: NetworkMap,
}

fn handle_keyboard_event(keyboard_event: keyboard::Event) -> Option<NetworkMapMessage> {
	match keyboard_event {
		keyboard::Event::KeyReleased { key_code, modifiers } => {
			match modifiers {
				_ if modifiers.is_empty() => {
					match key_code {
						keyboard::KeyCode::N => {
							return Some(NetworkMapMessage::CustomEvent(NetworkMapEvent::AddNetwork));
						}
						keyboard::KeyCode::M => {
							return Some(NetworkMapMessage::CustomEvent(NetworkMapEvent::AddMachine));
						}
						_ => None
					}
				}
				keyboard::Modifiers::CTRL => {
					match key_code {
						keyboard::KeyCode::S => {
							log::debug!("Triggered Save");
							return Some(NetworkMapMessage::CustomEvent(NetworkMapEvent::TriggerSave));
						}
						keyboard::KeyCode::R => {
							return Some(NetworkMapMessage::CustomEvent(NetworkMapEvent::TriggerReload));
						}
						keyboard::KeyCode::P => {
							return Some(NetworkMapMessage::CustomEvent(NetworkMapEvent::TriggerDebugPrint));
						}
						_ => None,
					}
				}
				_ => None
			}
		}
		_ => None
	}
}

impl NetworkTab {
	pub fn new() -> Self {
		Self {
			map: GraphWidget::new(handle_keyboard_event),
		}
	}
	pub fn clear(&mut self) {
		self.map = GraphWidget::new(handle_keyboard_event);
	}

	fn mouse_field_position(&self) -> FieldPosition {
		let cursor_pos = self.map.global_cursor_position;
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
			Message::UpdateMachine(id, info) => {},
    		Message::UpdateNetwork(id, info) => {},
			
			Message::UpdateConnection(wire_idx, from, to, activation) => {
				self.map.add_edge(NetworkTabEdge { id: wire_idx, source: from, dest: to, latency: 5 });
			},
			Message::RemoveConnection(wire_idx) => {
				self.map.remove_edge(wire_idx);
			}
			Message::MapMessage(map_msg) => {
				match map_msg {
					NetworkMapMessage::TriggerConnection(from, to) => {
						return Some(loaded::Message::ConnectNode(from, to));
					},
					NetworkMapMessage::NodeDragged(node, point) => {
						return Some(loaded::Message::MoveNode(node, FieldPosition::new(point.x as i32, point.y as i32)));
					}
					NetworkMapMessage::CustomEvent(event) => match event {
						NetworkMapEvent::AddMachine => return Some(loaded::Message::AddNode(self.mouse_field_position(), NodeType::Machine)),
						NetworkMapEvent::AddNetwork => return Some(loaded::Message::AddNode(self.mouse_field_position(), NodeType::Network)),
						NetworkMapEvent::TriggerSave => return Some(loaded::Message::TriggerSave),
						NetworkMapEvent::TriggerReload => return Some(loaded::Message::TriggerReload),
						NetworkMapEvent::TriggerDebugPrint => return Some(loaded::Message::DebugPrint),
					}
					_ => self.map.update(map_msg),
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

	fn content(&self) -> Element<'_, Self::Message> {
		container(
			column().push(self.map.view().map(move |message| Message::MapMessage(message)))
		).width(Length::Fill)
		.height(Length::Fill)
		.into()
	}
}


