#![allow(unused)]

use iced::{Align, Application, Column, Container, Element, Font, Length, Text};
use iced_aw::{TabBarPosition, TabLabel, Tabs};

const HEADER_SIZE: u16 = 32;
const TAB_PADDING: u16 = 16;

mod theme;

pub mod network_tab;
use network_tab::NetworkTab;

pub mod dither_tab;
use dither_tab::DitherTab;

use crate::gui::loaded;

const ICON_FONT: Font = iced::Font::External {
	name: "Icons",
	bytes: include_bytes!("./assets/icon_font.ttf"),
};

enum Icon {
	CentralizedNetwork,
	DistributedNetwork,
}

impl From<Icon> for char {
	fn from(icon: Icon) -> Self {
		match icon {
			Icon::CentralizedNetwork => 'B',
			Icon::DistributedNetwork => 'A',
		}
	}
}

pub struct TabBar {
	active_tab: usize,
	pub network_tab: NetworkTab,
	pub dither_tab: DitherTab,
}

#[derive(Clone, Debug)]
pub enum Message {
	TabSelected(usize),

	NetworkTab(network_tab::Message),
	DitherTab(dither_tab::Message),
}

impl TabBar {
	pub fn new() -> Self {
		TabBar {
			active_tab: 0,
			network_tab: NetworkTab::new(),
			dither_tab: DitherTab::new(),
		}
	}

	pub fn process(&mut self, message: Message) -> Option<loaded::Message> {
		match message {
			Message::TabSelected(selected) => self.active_tab = selected,
			Message::NetworkTab(message) => return self.network_tab.process(message),
			Message::DitherTab(message) => self.dither_tab.update(message),
		}
		None
	}

	pub fn view(&mut self) -> Element<'_, Message> {
		Tabs::new(self.active_tab, Message::TabSelected)
			.push(self.network_tab.tab_label(), self.network_tab.view().map(|m|Message::NetworkTab(m)))
			.push(self.dither_tab.tab_label(), self.dither_tab.view().map(|m|Message::DitherTab(m)))
			.tab_bar_style(theme::Theme::Default)
			.icon_font(ICON_FONT)
			.tab_bar_position(TabBarPosition::Top)
			.into()
	}
}

trait Tab {
	type Message;

	fn title(&self) -> String;

	fn tab_label(&self) -> TabLabel;

	fn view(&mut self) -> Element<'_, Self::Message> {
		let column = Column::new()
			//.push(Text::new(self.title()).size(HEADER_SIZE))
			.push(self.content());

		Container::new(column)
			.width(Length::Fill)
			.height(Length::Fill)
			.align_x(Align::Center)
			.align_y(Align::Center)
			//.padding(TAB_PADDING)
			.into()
	}

	fn content(&mut self) -> Element<'_, Self::Message>;
}
