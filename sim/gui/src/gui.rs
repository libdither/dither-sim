#![allow(dead_code)]

// use crate::graph::Graph;
pub use iced::Settings;
use iced::{Align, Application, Button, Column, Command, Clipboard, Container, Element, Row, Text, button, executor};
use iced::Checkbox;

use sim::Internet;

use crate::tabs::{self, TabBar};

#[derive(Default)]
pub struct TopBar {
	step_sim: button::State,
	toggle_sim: bool,
}

pub struct NetSimApp {
	internet: Internet,
	tabs: TabBar,
	top_bar: TopBar,
	//radius: f32,
	//slider: slider::State,
}

#[derive(Debug, Clone)]
pub enum Message {
	TabUpdate(tabs::Message),
	StepNetwork,
	ToggleRunning(bool),
}

pub struct NetSimAppSettings {
	pub net_sim: Internet,
}

impl Application for NetSimApp {
	type Executor = executor::Default;
	type Message = Message;
	type Flags = NetSimAppSettings;

	fn new(flags: NetSimAppSettings) -> (Self, Command<Self::Message>) {
		(NetSimApp {
			internet: flags.net_sim,
			tabs: TabBar::new(),
			top_bar: TopBar::default(),
		}, Command::none())
	}

	fn title(&self) -> String {
		String::from("Dither Network Simulation")
	}

	fn update(&mut self, message: Message, _clipboard: &mut Clipboard) -> Command<Self::Message> {
		let _rng = &mut rand::thread_rng();
		match message {
			Message::TabUpdate(tab_message) => self.tabs.update(tab_message),
			/* Message::StepNetwork => self.internet.tick(100, rng), */
			Message::ToggleRunning(toggle) => {
				self.top_bar.toggle_sim = toggle;
			}
			_ => {},
		}
		/* if self.top_bar.toggle_sim {
			self.internet.tick(1, rng);
		} */
		Command::none()
	}

	fn view(&mut self) -> Element<Message> {
		Column::new()
			.push(
				Row::new().push(
					Text::new("Top Bar")
				).push( // Step Network Button
					Button::new(&mut self.top_bar.step_sim, Text::new("Step Network"))
                    	.on_press(Message::StepNetwork)
				).push( // Run Network Continuously Toggle
					Checkbox::new(self.top_bar.toggle_sim, String::from("Run Network"), Message::ToggleRunning)
				).spacing(10).align_items(Align::Center).padding(3)
			).push(
				Container::new(
					self.tabs.view().map(move |m| Message::TabUpdate(m))
				)
			).into()
	}
	/* fn scale_factor(&self) -> f64 { 1.0 } */
}
