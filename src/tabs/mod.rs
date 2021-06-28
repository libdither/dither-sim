use iced::{Align, Application, Column, Container, Element, Font, Length, Text};
use iced_aw::{TabBarPosition, TabLabel, Tabs};

const HEADER_SIZE: u16 = 32;
const TAB_PADDING: u16 = 16;

mod theme;

mod network_tab;
use network_tab::{CounterMessage, CounterTab};

mod virtual_tab;
use virtual_tab::{LoginMessage, LoginTab};

const ICON_FONT: Font = iced::Font::External {
	name: "Icons",
	bytes: include_bytes!("icons.ttf"),
};

enum Icon {
	User,
	Heart,
	Calc,
	CogAlt,
}

impl From<Icon> for char {
	fn from(icon: Icon) -> Self {
		match icon {
			Icon::User => '\u{E800}',
			Icon::Heart => '\u{E801}',
			Icon::Calc => '\u{F1EC}',
			Icon::CogAlt => '\u{E802}',
		}
	}
}

pub struct TabBar {
	active_tab: usize,
	login_tab: LoginTab,
	counter_tab: CounterTab,
}

#[derive(Clone, Debug)]
pub enum Message {
	TabSelected(usize),
	Login(LoginMessage),
	Counter(CounterMessage),
}

impl TabBar {
	pub fn new() -> Self {
		TabBar {
			active_tab: 0,
			login_tab: LoginTab::new(),
			counter_tab: CounterTab::new(),
		}
	}

	pub fn update(&mut self, message: Message) {
		match message {
			Message::TabSelected(selected) => self.active_tab = selected,
			Message::Login(message) => self.login_tab.update(message),
			Message::Counter(message) => self.counter_tab.update(message),
		}
	}

	pub fn view(&mut self) -> Element<'_, Message> {
		/* let position = self
			.settings_tab
			.settings()
			.tab_bar_position
			.unwrap_or_default(); */

		Tabs::new(self.active_tab, Message::TabSelected)
			.push(self.login_tab.tab_label(), self.login_tab.view())
			.push(self.counter_tab.tab_label(), self.counter_tab.view())
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
			.spacing(20)
			.push(Text::new(self.title()).size(HEADER_SIZE))
			.push(self.content());

		Container::new(column)
			.width(Length::Fill)
			.height(Length::Fill)
			.align_x(Align::Center)
			.align_y(Align::Center)
			.padding(TAB_PADDING)
			.into()
	}

	fn content(&mut self) -> Element<'_, Self::Message>;
}
