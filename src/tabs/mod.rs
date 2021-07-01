#![allow(unused)]

use iced::{Align, Application, Column, Container, Element, Font, Length, Text};
use iced_aw::{TabBarPosition, TabLabel, Tabs};

const HEADER_SIZE: u16 = 32;
const TAB_PADDING: u16 = 16;

mod theme;

mod network_tab;
use network_tab::NetworkTab;

mod virtual_tab;
use virtual_tab::{LoginMessage, LoginTab};

const ICON_FONT: Font = iced::Font::External {
	name: "Icons",
	bytes: include_bytes!("../assets/icon_font.ttf"),
};

enum Icon {
	Centralized,
	Decentralized,
}

impl From<Icon> for char {
	fn from(icon: Icon) -> Self {
		match icon {
			Icon::Centralized => 'B',
			Icon::Decentralized => 'A',
		}
	}
}

pub struct TabBar {
	active_tab: usize,
	network_tab: NetworkTab,
	login_tab: LoginTab,
}

#[derive(Clone, Debug)]
pub enum Message {
	TabSelected(usize),
	Network(network_tab::Message),
	Login(LoginMessage),
}

impl TabBar {
	pub fn new() -> Self {
		TabBar {
			active_tab: 0,
			network_tab: NetworkTab::new(),
			login_tab: LoginTab::new(),
		}
	}

	pub fn update(&mut self, message: Message) {
		match message {
			Message::TabSelected(selected) => self.active_tab = selected,
			Message::Network(message) => self.network_tab.update(message),
			Message::Login(message) => self.login_tab.update(message),
		}
	}

	pub fn view(&mut self) -> Element<'_, Message> {
		/* let position = self
			.settings_tab
			.settings()
			.tab_bar_position
			.unwrap_or_default(); */

		Tabs::new(self.active_tab, Message::TabSelected)
			.push(self.network_tab.tab_label(), self.network_tab.view().map(|m|Message::Network(m)))
			.push(self.login_tab.tab_label(), self.login_tab.view())
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
