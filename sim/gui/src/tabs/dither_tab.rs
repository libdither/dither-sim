use iced::{
	button, text_input, Align, Button, Column, Container, Element, HorizontalAlignment, Length,
	Row, Text, TextInput,
};
use iced_aw::TabLabel;

use super::{Icon, Tab};

#[derive(Debug, Clone)]
pub enum Message {
	
}

pub struct DitherTab {
	username: String,
	username_state: text_input::State,
	password: String,
	password_state: text_input::State,
	clear_button: button::State,
	login_button: button::State,
}

impl DitherTab {
	pub fn new() -> Self {
		DitherTab {
			username: String::new(),
			username_state: text_input::State::default(),
			password: String::new(),
			password_state: text_input::State::default(),
			clear_button: button::State::default(),
			login_button: button::State::default(),
		}
	}

	pub fn update(&mut self, message: Message) {
		match message {
			_ => {},
		}
	}
}

impl Tab for DitherTab {
	type Message = Message;

	fn title(&self) -> String {
		String::from("Dither")
	}

	fn tab_label(&self) -> TabLabel {
		//TabLabel::Text(self.title())
		TabLabel::IconText(Icon::DistributedNetwork.into(), self.title())
	}

	fn content(&mut self) -> Element<'_, Self::Message> {
		let content: Element<'_, Message> = Container::new(
			Column::new()
		)
		.align_x(Align::Center)
		.align_y(Align::Center)
		.into();
		content
	}
}
