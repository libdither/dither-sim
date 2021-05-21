
use crate::graph::Graph;
pub use iced::Settings;
use iced::{Align, Application, Clipboard, Column, Command, Container, Element, Length, Slider, Text, executor, slider};

use sim::{internet::NetSim, node::Node};

pub struct NetSimApp {
	internet: NetSim<Node>,
	radius: f32,
	slider: slider::State,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
	RadiusChanged(f32),
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
			radius: 50.0,
			slider: slider::State::new(),
		}, Command::none())
	}

	fn title(&self) -> String {
		String::from("Dither Network Simulation")
	}

	fn update(&mut self, message: Message, _clipboard: &mut Clipboard) -> Command<Self::Message> {
		match message {
			Message::RadiusChanged(radius) => {
				self.radius = radius;
			}
		}
		Command::none()
	}

	fn view(&mut self) -> Element<Message> {
		let content = Column::new()
			.padding(0)
			.spacing(0)
			.max_width(2000)
			.align_items(Align::Center)
			.push(Graph::new(&self.internet))
			/* .push(Text::new(format!("Radius: {:.2}", self.radius)))
			.push(
				Slider::new(
					&mut self.slider,
					1.0..=100.0,
					self.radius,
					Message::RadiusChanged,
				)
				.step(0.01),
			) */;

		Container::new(content)
			.width(Length::Fill)
			.height(Length::Fill)
			.center_x()
			.center_y()
			.into()
	}
	fn scale_factor(&self) -> f64 {
		println!("Setting Scale Factor");
		1.0
	}
}
