use iced::{Align, Button, Checkbox, Column, Container, Element, Row, Text, Vector, button};
use sim::{FieldPosition, InternetAction, InternetEvent, NodeIdx, NodeType};
use futures::channel::mpsc;

use crate::{subscription::InternetRecipe, tabs::{self, TabBar, network_tab}};

#[derive(Default)]
pub struct TopBar {
	step_sim: button::State,
	toggle_sim: bool,
	add_machine: button::State,
	add_network: button::State,
}

pub struct State {
	pub internet_action: mpsc::Sender<InternetAction>,
	pub internet_recipe: InternetRecipe,

	pub tabs: TabBar,
	pub top_bar: TopBar,

	pub field_position: FieldPosition,
}

#[derive(Debug, Clone)]
pub enum Message {
	/// From Internet, forwarded via main gui
	InternetEvent(InternetEvent),

	/// From view
	TabUpdate(tabs::Message),
	ToggleSim(bool),
	TriggerAddMachine, // Sends AddMachine action to network with current field position state
	TriggerAddNetwork,

	/// From tabs & loaded gui
	RemoveNode(NodeIdx),
	MoveNode(NodeIdx, FieldPosition),
	ConnectNode(NodeIdx, NodeIdx),
	MousePosition(Vector),
	AddNode(FieldPosition, NodeType)
}

impl State {
	pub fn new(internet_action: mpsc::Sender<InternetAction>, internet_recipe: InternetRecipe) -> Self {
		Self {
			internet_action,
			internet_recipe,
			tabs: TabBar::new(),
			top_bar: TopBar::default(),
			field_position: Default::default(),
		}
	}
	pub fn net_action(&mut self, action: InternetAction) {
		match self.internet_action.try_send(action) {
			Err(err) => log::error!("Failed to send internet action: {}", err), Ok(_) => {},
		}
	}
	fn process_tabmsg(&mut self, tabmsg: tabs::Message) -> Option<super::Message> {
		self.tabs.process(tabmsg).map(|msg|self.process(msg)).flatten()
	}
	fn process_network_tab_msg(&mut self, network_msg: network_tab::Message) -> Option<super::Message> {
		self.process_tabmsg(tabs::Message::NetworkTab(network_msg))
	}
	pub fn process(&mut self, message: Message) -> Option<super::Message> {
		match message {
			// Handle internet events
			Message::InternetEvent(internet_event) => {
				log::debug!("received internet event: {:?}", internet_event);
				match internet_event {
					InternetEvent::NewMachine(id) => {
						self.process_network_tab_msg(network_tab::Message::AddNode(id, NodeType::Machine))
					},
					InternetEvent::NewNetwork(id) => {
						self.process_network_tab_msg(network_tab::Message::AddNode(id, NodeType::Network))
					},
					InternetEvent::NodeInfo(id, info) => {
						self.process_network_tab_msg(network_tab::Message::UpdateNode(id, info))
					},
					InternetEvent::MachineInfo(id, info) => {
						self.process_network_tab_msg(network_tab::Message::UpdateMachine(id, info))
					},
					InternetEvent::NetworkInfo(id, info) => {
						self.process_network_tab_msg(network_tab::Message::UpdateNetwork(id, info))
					},
					InternetEvent::ConnectionInfo(wire_idx, from, to) => {
						self.process_network_tab_msg(network_tab::Message::UpdateConnection(wire_idx, from, to, true))
					}
					InternetEvent::Error(err) => { match *err {
						sim::InternetError::NodeConnectionError => { log::error!("Internet Error: Cannot connect two machines to each other"); },
						_ => todo!(),
					} None },
					//_ => { println!("Received Internet Event: {:?}", internet_event) }
				}
			}
			// Forward Tab events
			Message::TabUpdate(tab_message) => {
				self.process_tabmsg(tab_message)
			},
			// Handle button events
			Message::ToggleSim(toggle) => {
				self.top_bar.toggle_sim = toggle;
				None
			}
			Message::TriggerAddMachine => {
				None
			},
			Message::TriggerAddNetwork => {
				self.net_action(InternetAction::AddNetwork(self.field_position));
				None
			},
			// Handle general events
			Message::MoveNode(index, new_position) => {
				self.net_action(InternetAction::SetPosition(index, new_position));
				None
			},
			Message::ConnectNode(from, to) => {
				self.net_action(InternetAction::ConnectNodes(from, to)); None
			}
			Message::AddNode(position, node_type) => {
				match node_type {
					NodeType::Machine => self.net_action(InternetAction::AddMachine(position)),
					NodeType::Network => self.net_action(InternetAction::AddNetwork(position)),
				}
				None
			},
			_ => { log::warn!("received unimplemented loaded::Message: {:?}", message); None }
		}
	}
	pub fn view(&mut self) -> Element<Message> {
		Column::new()
			.push(
				Row::new().push(
					Text::new("Top Bar")
				)/* .push( // Step Network Button
					Button::new(&mut self.top_bar.step_sim, Text::new("Step Network"))
                    	.on_press(Message::StepNetwork)
				) */.push( // Run Network Continuously Toggle
					Checkbox::new(self.top_bar.toggle_sim, String::from("Run Network"), Message::ToggleSim)
				).spacing(10).align_items(Align::Center).padding(3)
				.push(
					Button::new(&mut self.top_bar.add_machine, Text::new("Add Machine"))
        			.on_press(Message::TriggerAddMachine)
				).push(
					Button::new(&mut self.top_bar.add_network, Text::new("Add Network"))
        			.on_press(Message::TriggerAddNetwork)
				)/* .push(
					Text::new(format!("({}, {})", self.tabs.network_tab.map.translation.x, self.tabs.network_tab.map.translation.y))
				) */
			).push(
				Container::new(
					self.tabs.view().map(move |m| Message::TabUpdate(m))
				)
			).into()
	}
}
