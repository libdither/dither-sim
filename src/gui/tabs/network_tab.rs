use super::{Icon, Tab};
use iced::{button, Align, Button, Column, Container, Element, Length, Row, Text};
use iced_aw::TabLabel;

use crate::gui::network_map::{self, NetworkMap};

#[derive(Debug, Clone)]
pub enum Message {
	NetMap(network_map::Message),
}

pub struct NetworkTab {
	map: NetworkMap,
}

impl NetworkTab {
	pub fn new() -> Self {
		Self {
			map: NetworkMap::test_conf(),
		}
	}

	pub fn update(&mut self, message: Message) {
		/* match message {
			CounterMessage::Increase => self.value += 1,
			CounterMessage::Decrease => self.value -= 1,
		} */
	}
}

impl Tab for NetworkTab {
	type Message = Message;

	fn title(&self) -> String {
		String::from("Internet")
	}

	fn tab_label(&self) -> TabLabel {
		//TabLabel::Text(self.title())
		TabLabel::IconText(Icon::CentralizedNetwork.into(), self.title())
	}

	fn content(&mut self) -> Element<'_, Self::Message> {
		let content = Column::new().push(self.map.view().map(move |message| Message::NetMap(message)));

		Container::new(content)
			.width(Length::Fill)
			.height(Length::Fill)
			.into()
	}
}


