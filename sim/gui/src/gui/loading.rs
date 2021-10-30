use iced::{button, text_input, Button, Column, Element, Text, TextInput};

use crate::subscription::InternetRecipe;

#[derive(Default)]
pub struct State {
	load_input: text_input::State,
	load_button: button::State,
	network_file: Option<String>,
	pub currently_loading_recipe: Option<InternetRecipe>,
}

#[derive(Debug, Clone)]
pub enum Message {
	LocateFile(String),
	TriggerLoad,
}

impl State {
	pub fn process(&mut self, message: Message) -> Option<super::Message> {
		match message {
			Message::LocateFile(string) => {
				if std::path::Path::new(&string).exists() {
					self.network_file = Some(string);
				}
				None
			}
			Message::TriggerLoad => {
				self.currently_loading_recipe = Some(InternetRecipe { path: self.network_file.clone() });
				Some(super::Message::LoadInternet)
			},
		}
	}

	pub fn view(&mut self) -> Element<Message> {
		Column::new()
			.push(TextInput::new(
				&mut self.load_input,
				"Simulation File, i.e. simulation.bin",
				"",
				|string| Message::LocateFile(string),
			))
			.push(
				Button::new(&mut self.load_button, Text::new("Load Simulation"))
					.on_press(Message::TriggerLoad),
			)
			.into()
	}
}
