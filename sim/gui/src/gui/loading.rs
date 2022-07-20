use iced::{Alignment, pure::{Element, button, column, row, text, text_input}};

use crate::subscription::InternetRecipe;

#[derive(Default)]
pub struct State {
	pub text_input_string: String,
	pub valid_file: bool,

	pub currently_loading_recipe: Option<InternetRecipe>,
}

#[derive(Debug, Clone)]
pub enum Message {
	TextBoxUpdate(String),
	TriggerLoad,
}

impl State {
	pub fn process(&mut self, message: Message) -> Option<super::Message> {
		match message {
			Message::TextBoxUpdate(string) => {
				self.text_input_string = string;
				self.valid_file = std::path::Path::new(&self.text_input_string).exists();
				None
			}
			Message::TriggerLoad => {
				self.currently_loading_recipe = Some(InternetRecipe {
					path: self.valid_file.then(|| self.text_input_string.clone())
				});
				Some(super::Message::LoadInternet)
			},
		}
	}

	pub fn view(&self) -> Element<Message> {
		column().align_items(Alignment::Center).padding(20).spacing(20).push(
			row()
				.push(text_input("Simulation Binary File", &self.text_input_string, |string| Message::TextBoxUpdate(string),))
		.push(text(if self.valid_file { "Valid" } else { "Unknown File" }))
		).push(
			button("Load Simulation").on_press(Message::TriggerLoad),
		).into()
	}
}
