use std::net::Ipv4Addr;

use super::{Icon, Tab};
use iced::{Align, Button, Color, Column, Container, Element, Length, Row, Text, Vector, button};
use iced_aw::TabLabel;
use petgraph::Undirected;
use sim::{FieldPosition, NodeType};

use crate::network_map::{self, NetworkEdge, NetworkMap, NetworkNode};

#[derive(Clone, Debug)]
pub struct NetworkTabNode {
	id: usize,
	node_type: NodeType,
	field_position: FieldPosition,
	ip_addr: Ipv4Addr,
	
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
	AddNode(NetworkTabNode),
	RemoveNode(usize), // Removes edges too.

	NetMap(network_map::Message),
}

pub struct NetworkTab {
	map: NetworkMap<NetworkTabNode, NetworkTabEdge, Undirected>,
}

impl NetworkTab {
	pub fn new() -> Self {
		Self {
			map: NetworkMap::test_conf(),
		}
	}

	pub fn update(&mut self, message: Message) {
		match message {
			Message::AddNode(node) => {
				self.map.add_node(node);
			},
			Message::RemoveNode(idx) => {
				self.map.remove_node(idx);
			},
			Message::NetMap(netmap_msg) => {
				todo!();
				/* match netmap_msg {
					.
				}
				self.map.update(netmap_msg).map(|m|self.update(Message::NetMap(m))); */
			},
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
		let content = Column::new().push(self.map.view().map(move |message| Message::NetMap(message)));

		Container::new(content)
			.width(Length::Fill)
			.height(Length::Fill)
			.into()
	}
}


