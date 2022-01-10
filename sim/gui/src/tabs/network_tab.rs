use std::net::Ipv4Addr;

use super::{Icon, Tab};
use iced::{Align, Button, Color, Column, Container, Element, Length, Row, Text, Vector, button};
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
	fn color(&self) -> Color {
		match self.node_type {
			NodeType::Machine => Color::BLACK,
			NodeType::Network => Color::from_rgb(0.5, 0.5, 0.5),
		}
		
	}
	fn size(&self) -> u32 {
		30
	}
	fn position(&self) -> Vector {
		
		Vector::new(self.field_position.x as f32, self.field_position.y as f32)
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

	pub fn update(&mut self, message: Message) {
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
			_ => {}
			// Handled by 
			/* Message::NetMapMessage(netmap_msg) => {
				use crate::network_map::Message as MapMsg;
				match netmap_msg {
					MapMsg::TriggerNewNode(point) => return loaded::Message::TriggerAddMachine(point)
				}
			}, */
		}
	}
}

impl Tab for NetworkTab {
	type Message = Message;

	fn title(&self) -> String {
		String::from("Internet")
	}

	fn tab_label(&self) -> TabLabel {
		//TabLabel::Text(self.title())
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


