use std::net::Ipv4Addr;

use super::{Icon, Tab};
use anyhow::Context;
use iced::{Align, Button, Color, Column, Container, Element, Length, Point, Row, Text, Vector, button, canvas::{self, Path, Stroke, event}, keyboard};
use iced_aw::TabLabel;
use libdither::{DitherCommand, Multiaddr};
use petgraph::Undirected;
use sim::{FieldPosition, MachineInfo, NodeID, NodeIdx, NodeType, RouteCoord, WireIdx, node::net::Address};

use crate::{gui::loaded, network_map::{self, NetworkEdge, NetworkMap, NetworkNode}};

#[derive(Clone, Debug)]
pub struct DitherTabNode {
	id: NodeIdx,
	node_id: NodeID,
	route_coord: RouteCoord,
	known_self_addr: Option<Address>,
	network_ip: Option<Ipv4Addr>,
}
impl DitherTabNode {
	fn new(id: NodeIdx, info: MachineInfo, index: usize) -> DitherTabNode {
		Self {
			id,
			node_id: info.node_id,
			route_coord: info.route_coord.unwrap_or((index as i64 * 60, 0)),
			known_self_addr: info.public_addr,
			network_ip: info.network_ip,
		}
	}
}
impl NetworkNode for DitherTabNode {
	type NodeId = NodeIdx;
	fn unique_id(&self) -> Self::NodeId { self.id }
	/* Alternate Disp syntax
	position = λ(self) {
		> self.route_coord map { Vector::new (f32 x) (f32 y) } unwrap_default
	}
	position = λ(self) > self.route_coord map { Vector::new (f32 x) (f32 y) } unwrap_default
	*/
	fn position(&self) -> Vector {
		Vector::new(self.route_coord.0 as f32, self.route_coord.1 as f32)
	}
	fn render(&self, frame: &mut canvas::Frame, hover: bool, selected: bool, scaling: f32) {
		let point = {
			let position = self.position();
			Point::new(position.x, position.y)
		};
		let radius = 30.0;
		
		if selected {
			frame.fill(&Path::circle(point.clone(), radius + 5.0), Color::from_rgb8(255, 255, 0));
		}

		let mut node_color = Color::from_rgb8(0, 0, 0);
		if hover { node_color = Color::from_rgb8(200, 200, 200); }
		frame.fill(&Path::circle(point.clone(), radius), node_color);

		frame.fill_text(canvas::Text { content:
			format!("ID: {}",
			self.id),
			position: point, color: Color::BLACK, size: radius * scaling,
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
pub struct DitherTabEdge {
	pub id: WireIdx,
	pub source: NodeIdx,
	pub dest: NodeIdx,
	pub latency: usize,
}
impl NetworkEdge<DitherTabNode> for DitherTabEdge {
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
	UpdateMachine(NodeIdx, sim::MachineInfo),
	UpdateConnection(WireIdx, NodeIdx, NodeIdx, bool),
	RemoveConnection(WireIdx),
	RemoveNode(NodeIdx), // Removes edges too.

	NetMapMessage(network_map::Message<DitherTabNode, DitherTabEdge>),
}

pub struct DitherTab {
	map: NetworkMap<DitherTabNode, DitherTabEdge, Undirected>,
}

impl DitherTab {
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
		let ret: anyhow::Result<Option<loaded::Message>> = try {
			match message {
				// This tab only pays attention to MachineUpdates propagated from network-sandboxed nodes
				Message::UpdateMachine(id, info) => {
					// Get node or add it if doesn't exist.
					if let Some(node) = self.map.node_mut(id) { node } else {
						self.map.add_node(DitherTabNode::new(id, info, self.map.nodes.node_count()));
						self.map.node_mut(id).unwrap()
					};
					self.map.trigger_update();
				},
				Message::RemoveNode(idx) => {
					self.map.remove_node(idx);
				},
				
				Message::UpdateConnection(wire_idx, from, to, activation) => {
					self.map.add_edge(DitherTabEdge { id: wire_idx, source: from, dest: to, latency: 5 });
				},
				Message::RemoveConnection(wire_idx) => {
					self.map.remove_edge(wire_idx);
				}
				Message::NetMapMessage(netmap_msg) => {
					match netmap_msg {
						network_map::Message::TriggerConnection(from, to) => {
							let node = self.map.node(to).ok_or(anyhow!("No node: {}", to))?;
							let network_ip = node.network_ip.clone().ok_or(anyhow!("Node {:?} does not have a network ip", to))?;
							let network_ip = format!("/ip4/{}/tcp/3000", network_ip).parse::<Multiaddr>().context("cannot parse multihash")?;
							let network_ip = Address(network_ip.to_vec());
							
							log::debug!("Connecting node: {:?} to {:?}", from, node);
							return Some(loaded::Message::DitherCommand(from, DitherCommand::Bootstrap(node.node_id.clone(), network_ip)));
						},
						network_map::Message::CanvasEvent(canvas::Event::Keyboard(keyboard_event)) => {
							match keyboard_event {
								keyboard::Event::KeyReleased { key_code, modifiers } => {
									match modifiers {
										keyboard::Modifiers { shift: false, control: false, alt: false, logo: false } => {
											match key_code {
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
		};
		if let Err(err) = ret {
			Some(loaded::Message::DisplayError(format!("{err}")))
		} else { None }
	}
}

impl Tab for DitherTab {
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


