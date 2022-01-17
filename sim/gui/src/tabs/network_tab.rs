use std::net::Ipv4Addr;

use super::{Icon, Tab};
use iced::{Align, Button, Color, Column, Container, Element, Length, Point, Row, Text, Vector, button, canvas::{self, Path, event}, keyboard};
use iced_aw::TabLabel;
use petgraph::Undirected;
use sim::{FieldPosition, NodeType};

use crate::{gui::loaded, network_map::{self, NetworkEdge, NetworkMap, NetworkNode}};

#[derive(Clone, Debug)]
pub struct NetworkTabNode {
	id: usize,
	node_type: NodeType,
	field_position: FieldPosition,
	ip_addr: Option<Ipv4Addr>,
}
impl NetworkTabNode {
	fn new(id: usize, node_type: NodeType) -> NetworkTabNode {
		Self { id, node_type, field_position: Default::default(), ip_addr: None }
	}
}
impl NetworkNode for NetworkTabNode {
	fn unique_id(&self) -> usize {
		self.id
	}
	fn position(&self) -> Vector {
		Vector::new(self.field_position.x as f32, self.field_position.y as f32)
	}
	fn render(&self, frame: &mut canvas::Frame, hover: bool, selected: bool) {
		let point = {
			Point::new(self.field_position.x as f32, self.field_position.y as f32)
		};
		let radius = 30.0;
		
		if selected {
			frame.fill(&Path::circle(point.clone(), radius + 5.0), Color::from_rgb8(255, 255, 0));
		}

		let node_color = if hover { Color::from_rgb8(200, 200, 200) } else { Color::BLACK };
		frame.fill(&Path::circle(point.clone(), radius), node_color);

		frame.fill_text(canvas::Text { content:
			format!("ID: {}",
			self.id),
			position: point, color: Color::from_rgb8(255, 0, 0), size: 20.0,
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
	pub source: usize,
	pub dest: usize,
	pub latency: usize,
}
impl NetworkEdge for NetworkTabEdge {
	fn color(&self) -> Color {
		Color::BLACK
	}
	fn width(&self) -> u8 {
		5
	}
	fn unique_connection(&self) -> (usize, usize) {
		(self.source, self.dest)
	}
}

#[derive(Debug, Clone)]
pub enum Message {
	AddNode(usize, sim::NodeType),
	UpdateNode(usize, sim::NodeInfo),
	UpdateMachine(usize, sim::MachineInfo),
	UpdateNetwork(usize, sim::NetworkInfo),
	RemoveNode(usize), // Removes edges too.

	NetMapMessage(network_map::Message),
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

	fn mouse_field_position(&self) -> FieldPosition {
		let cursor_pos = self.map.global_cursor_position();
		FieldPosition::new(cursor_pos.x as i32, cursor_pos.y as i32)
	}

	pub fn process(&mut self, message: Message) -> Option<loaded::Message> {
		match message {
			Message::AddNode(id, node_type) => {
				self.map.add_node(NetworkTabNode::new(id, node_type));
			},
			Message::UpdateNode(id, info) => {
				let node = self.map.node_mut(id).unwrap();
				node.field_position = info.position;
				node.ip_addr = info.local_address;
			}
			Message::UpdateMachine(_, _) => {},
    		Message::UpdateNetwork(_, _) => {},
			Message::RemoveNode(idx) => {
				self.map.remove_node(idx);
			},
			Message::NetMapMessage(netmap_msg) => {
				match netmap_msg {
					network_map::Message::TriggerConnection(from, to) => {
						println!("triggered connectiong from {} to {}", from, to);
					},
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
											_ => {}
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


