use iced::{Align, Button, Checkbox, Column, Container, Element, Row, Text, button};
use sim::InternetAction;
use tokio::sync::mpsc;

use crate::{subscription::InternetRecipe, tabs::{self, TabBar}};

#[derive(Default)]
pub struct TopBar {
	step_sim: button::State,
	toggle_sim: bool,
	add_node: button::State,
}

pub struct State {
	pub internet_action: mpsc::Sender<InternetAction>,
	pub internet_recipe: InternetRecipe,

	pub tabs: TabBar,
	pub top_bar: TopBar,
}

#[derive(Debug, Clone)]
pub enum Message {
	TabUpdate(tabs::Message),
	StepNetwork,
	ToggleRunning(bool),
	AddNode,
}

impl State {
	pub fn new(internet_action: mpsc::Sender<InternetAction>, internet_recipe: InternetRecipe) -> Self {
		Self {
			internet_action,
			internet_recipe,
			tabs: TabBar::new(),
			top_bar: TopBar::default(),
		}
	}
	pub fn process(&mut self, message: Message) -> Option<super::Message> {
		match message {
			Message::TabUpdate(tab_message) => { self.tabs.update(tab_message); None },
			Message::StepNetwork => { println!("Step Network"); None },
			Message::ToggleRunning(toggle) => {
				self.top_bar.toggle_sim = toggle;
				None
			}
			Message::AddNode => {
				Some(super::Message::InternetAction(InternetAction::AddNode))
			}
		}
	}
	pub fn view(&mut self) -> Element<Message> {
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
				.push(
					Button::new(&mut self.top_bar.add_node, Text::new("Add Node"))
        			.on_press(Message::AddNode)
				)
			).push(
				Container::new(
					self.tabs.view().map(move |m| Message::TabUpdate(m))
				)
			).into()
	}
}
