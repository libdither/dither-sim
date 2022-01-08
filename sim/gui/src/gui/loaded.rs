use iced::{Align, Button, Checkbox, Column, Container, Element, Row, Text, Vector, button};
use sim::{FieldPosition, InternetAction, InternetEvent, NodeType};
use futures::channel::mpsc;

use crate::{network_map, subscription::InternetRecipe, tabs::{self, TabBar, network_tab}};

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
	RemoveNode(usize),
	MoveNode(usize, FieldPosition),
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
	pub fn process(&mut self, message: Message) -> Option<super::Message> {
		match message {
			// Handle internet events
			Message::InternetEvent(internet_event) => {
				match internet_event {
					InternetEvent::NewMachine(id) => {
						self.tabs.update(tabs::Message::NetworkTab(network_tab::Message::AddNode(id, NodeType::Machine))); None
					},
					InternetEvent::NewNetwork(id) => {
						self.tabs.update(tabs::Message::NetworkTab(network_tab::Message::AddNode(id, NodeType::Network))); None
					},
					InternetEvent::NodeInfo(id, info) => {
						let info = info.expect("expected info");
						self.tabs.update(tabs::Message::NetworkTab(network_tab::Message::UpdateNode(id, info))); None
					},
					InternetEvent::MachineInfo(id, machine_info) => {
						let info = machine_info.expect("expected machine info");
						self.tabs.update(tabs::Message::NetworkTab(network_tab::Message::UpdateMachine(id, info))); None
					},
					InternetEvent::NetworkInfo(id, network_info) => {
						let info = network_info.expect("expected network info");
						self.tabs.update(tabs::Message::NetworkTab(network_tab::Message::UpdateNetwork(id, info))); None
					},
					InternetEvent::Error(_) => todo!(),
					//_ => { println!("Received Internet Event: {:?}", internet_event) }
				}
			}
			// Forward Tab events
			Message::TabUpdate(tab_message) => {
				match &tab_message {
					tabs::Message::NetworkTab(network_tab_message) => match network_tab_message {
						network_tab::Message::NetMapMessage(netmap_msg) => match netmap_msg {
							network_map::Message::TriggerNewNode(point) => {
								let field_position = FieldPosition::new(point.x as i32, point.y as i32);
								return self.process(Message::AddNode(field_position, NodeType::Machine));
							}
							_ => {},
						}
						_ => {}
					}
					_ => {},
				}
				self.tabs.update(tab_message); None
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
				).push(
					Text::new(format!("({}, {})", self.field_position.x, self.field_position.y))
				)
			).push(
				Container::new(
					self.tabs.view().map(move |m| Message::TabUpdate(m))
				)
			).into()
	}
}
