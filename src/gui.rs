
// use crate::graph::Graph;
pub use iced::Settings;
use iced::{Align, Application, Clipboard, Column, Command, Container, Element, Length, executor};
use crate::tabs::{self, TabBar};

use sim::{internet::NetSim, node::Node};

pub struct NetSimApp {
	internet: NetSim<Node>,
	tabs: TabBar,
	//radius: f32,
	//slider: slider::State,
}

#[derive(Debug, Clone)]
pub enum Message {
	TabUpdate(tabs::Message),
}

pub struct NetSimAppSettings {
	pub net_sim: NetSim<Node>
}

impl Application for NetSimApp {
	type Executor = executor::Default;
	type Message = Message;
	type Flags = NetSimAppSettings;

	fn new(flags: NetSimAppSettings) -> (Self, Command<Self::Message>) {
		(NetSimApp {
			internet: flags.net_sim,
			tabs: TabBar::new(),
			//radius: 50.0,
			//slider: slider::State::new(),
		}, Command::none())
	}

	fn title(&self) -> String {
		String::from("Dither Network Simulation")
	}

	fn update(&mut self, message: Message, _clipboard: &mut Clipboard) -> Command<Self::Message> {
		match message {
			Message::TabUpdate(message) => self.tabs.update(message),
		}
		Command::none()
	}

	fn view(&mut self) -> Element<Message> {
		// Present Tabs
		self.tabs.view().map(move |m| Message::TabUpdate(m))
	}
	fn scale_factor(&self) -> f64 {
		//println!("Setting Scale Factor");
		1.0
	}
}
