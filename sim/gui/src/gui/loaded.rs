use iced::pure::{container, column, text_input, Element};
use libdither::DitherCommand;
use sim::{FieldPosition, InternetAction, InternetEvent, NodeIdx, NodeType};
use futures::channel::mpsc;

use crate::{subscription::InternetRecipe, tabs::{self, TabBar, dither_tab, network_tab}};

#[derive(Default)]
pub struct TopBar {
	toggle_sim: bool,
	action_box_text: String,
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
	ActionBoxUpdate(String),
	ActionBoxSubmit,

	TriggerAddMachine, // Sends AddMachine action to network with current field position state
	TriggerAddNetwork,
	TriggerSave,
	TriggerReload,
	DebugPrint,

	/// From tabs & loaded gui
	RemoveNode(NodeIdx),
	MoveNode(NodeIdx, FieldPosition),
	ConnectNode(NodeIdx, NodeIdx),
	DitherCommand(NodeIdx, DitherCommand),
	AddNode(FieldPosition, NodeType),
	DisplayError(String),
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
	fn process_dither_tab_msg(&mut self, dither_msg: dither_tab::Message) -> Option<super::Message> {
		self.process_tabmsg(tabs::Message::DitherTab(dither_msg))
	}
	pub fn process(&mut self, message: Message) -> Option<super::Message> {
		match message {
			// Handle internet events
			Message::InternetEvent(internet_event) => {
				log::debug!("received InternetEvent: {:?}", internet_event);
				match internet_event {
					InternetEvent::ClearUI => {
						self.tabs.network_tab.clear(); None
					},
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
						// self.process_network_tab_msg(network_tab::Message::UpdateMachine(id, info))
						self.process_dither_tab_msg(dither_tab::Message::UpdateMachine(id, info))
					},
					InternetEvent::NetworkInfo(id, info) => {
						self.process_network_tab_msg(network_tab::Message::UpdateNetwork(id, info))
					},
					InternetEvent::ConnectionInfo(wire_idx, from, to) => {
						self.process_network_tab_msg(network_tab::Message::UpdateConnection(wire_idx, from, to, true))
					}
					InternetEvent::RemoveConnection(wire_idx) => {
						self.process_network_tab_msg(network_tab::Message::RemoveConnection(wire_idx))
					}
					InternetEvent::Error(err) => { match *err {
						sim::InternetError::NodeConnectionError => { log::warn!("Internet Error: Cannot connect two machines to each other"); },
						_ => log::error!("received InternetError: {}", *err),
					} None },
					//_ => { println!("Received Internet Event: {:?}", internet_event) }
				}
			}
			// Forward Tab events
			Message::TabUpdate(tab_message) => {
				self.process_tabmsg(tab_message)
			},
			// Locally triggered events
			Message::ToggleSim(toggle) => {
				self.top_bar.toggle_sim = toggle;
				None
			}
			Message::ActionBoxUpdate(string) => {
				self.top_bar.action_box_text = string;
				None
			}
			Message::ActionBoxSubmit => {
				match ron::from_str(&self.top_bar.action_box_text) {
					Ok(action) => {
						self.net_action(action);
						self.top_bar.action_box_text.clear();
					}
					Err(err) => log::error!("Failed to parse Internet Action: {}", err)
				}
				None
			}

			// Externally triggered events
			Message::TriggerSave => {
				log::debug!("Saving Network to: {:?}", self.internet_recipe.path);
				if let Some(path) = &self.internet_recipe.path {
					self.net_action(InternetAction::SaveInternet(path.clone()));
				}
				 None
			}
			Message::TriggerReload => {
				self.net_action(InternetAction::RequestAllNodes); None
			}
			Message::DebugPrint => { self.net_action(InternetAction::DebugPrint); None }
			// Handle general events
			Message::MoveNode(index, new_position) => {
				self.net_action(InternetAction::SetPosition(index, new_position));
				None
			},
			Message::ConnectNode(from, to) => {
				self.net_action(InternetAction::ConnectNodes(from, to)); None
			}
			Message::DitherCommand(node_idx, command) => {
				self.net_action(InternetAction::DitherCommand(node_idx, command)); None
			}
			Message::AddNode(position, node_type) => {
				match node_type {
					NodeType::Machine => self.net_action(InternetAction::AddMachine(position)),
					NodeType::Network => self.net_action(InternetAction::AddNetwork(position)),
				}
				None
			},
			Message::DisplayError(string) => {
				log::error!("Tab Error: {}", string); None
			},
			_ => { log::warn!("received unimplemented loaded::Message: {:?}", message); None }
		}
	}
	pub fn view(&self) -> Element<Message> {
		column()
			/* .push(
				Row::new().push(
					Text::new("Top Bar")
				)/* .push( // Step Network Button
					Button::new(&mut self.top_bar.step_sim, Text::new("Step Network"))
                    	.on_press(Message::StepNetwork)
				) */.push( // Run Network Continuously Toggle
					Checkbox::new(self.top_bar.toggle_sim, String::from("Run Network"), Message::ToggleSim)
				).spacing(10).align_items(Align::Center).padding(3)
				/* .push(
					Button::new(&mut self.top_bar.add_machine, Text::new("Add Machine"))
        			.on_press(Message::TriggerAddMachine)
				).push(
					Button::new(&mut self.top_bar.add_network, Text::new("Add Network"))
        			.on_press(Message::TriggerAddNetwork)
				).push(
					Text::new(format!("({}, {})", self.tabs.network_tab.map.translation.x, self.tabs.network_tab.map.translation.y))
				) */
			) */
			.push(
				text_input("DebugPrint", &self.top_bar.action_box_text, Message::ActionBoxUpdate)
				.on_submit(Message::ActionBoxSubmit)
			)
			.push(
				container(
					self.tabs.view().map(move |m| Message::TabUpdate(m))
				)
			).into()
	}
}
