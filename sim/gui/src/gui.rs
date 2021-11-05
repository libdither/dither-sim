#![allow(dead_code)]

pub use iced::Settings;
use iced::{executor, Application, Clipboard, Command, Element, Subscription};

use sim::InternetAction;

use crate::subscription::{self, Event};

mod loaded;
mod loading;

pub struct NetSimAppSettings {}

pub enum NetSimApp {
	Loading(loading::State),
	Loaded(loaded::State),
}

#[derive(Debug)]
pub enum Message {
	LoadingMessage(loading::Message),
	LoadInternet,

	LoadedMessage(loaded::Message),
	InternetEvent(subscription::Event),
	InternetAction(InternetAction),
}

impl Application for NetSimApp {
	type Executor = executor::Default;
	type Message = Message;
	type Flags = NetSimAppSettings;

	fn new(_flags: NetSimAppSettings) -> (Self, Command<Self::Message>) {
		(
			NetSimApp::Loading(loading::State::default()),
			Command::none(),
		)
	}

	fn title(&self) -> String {
		String::from("Dither Network Simulation")
	}

	fn subscription(&self) -> Subscription<Message> {
		match self {
			NetSimApp::Loading(state) => {
				if let Some(recipe) = &state.currently_loading_recipe {
					Subscription::from_recipe(recipe.clone()).map(|event| Message::InternetEvent(event))
				} else {
					Subscription::none()
				}
			}
			// State is changed to Loading once Init Event triggered from Subscription
			NetSimApp::Loaded(state) => Subscription::from_recipe(state.internet_recipe.clone())
				.map(|event| Message::InternetEvent(event)),
		}
	}

	fn update(&mut self, message: Message, clipboard: &mut Clipboard) -> Command<Self::Message> {
		match self {
			NetSimApp::Loading(state) => {
				log::trace!("[Loading] Received Message: {:?}", message);
				match message {
					Message::LoadingMessage(message) => {
						if let Some(message) = state.process(message) {
							self.update(message, clipboard);
						}
					}
					Message::LoadInternet => {
						println!("Loading Internet: {:?}", state.currently_loading_recipe);
					}
					Message::InternetEvent(event) => {
						match event {
							Event::Init(sender) => {
								*self = NetSimApp::Loaded(loaded::State::new(
									sender,
									state.currently_loading_recipe.take().expect("There should be a recipe if internet is loaded")
								));
							}
							_ => log::error!("Received internet event {:?} in Loading state, state should have already switched to Loaded", event)
						}
					}
					_ => log::error!("Received Message: {:?} but inapplicable to loading state", message)
				}
			}
			NetSimApp::Loaded(state) => {
				log::trace!("[Loaded] Received Message: {:?}", message);
				match message {
					Message::LoadedMessage(loaded_message) => {
						if let Some(message) = state.process(loaded_message) {
							self.update(message, clipboard);
						}
					}
					Message::InternetAction(action) => {
						if let Err(err) = state.internet_action.try_send(action) {
							log::error!("Couldn't send action to simulation: {:?}", err);
						}
					}
					Message::InternetEvent(event) => {
						match event {
							Event::Init(sender) => state.internet_action = sender,
							Event::Event(internet_event) => {
								match internet_event {
									_ => { println!("Received Internet Event: {:?}", internet_event) }
								}
							}
							Event::Error(err) => log::error!("Internet Sim errored: {:?}", err),
							Event::Closed => {
								log::info!("Internet Sim closed");
								*self = NetSimApp::Loading(loading::State::default());
							},
						}
					}
					_ => log::error!("Received Message: {:?} but inapplicable to loaded state", message)
				}
			}
		}
		Command::none()
	}

	fn view(&mut self) -> Element<Message> {
		match self {
			NetSimApp::Loading(state) => state.view().map(|msg| Message::LoadingMessage(msg)),
			NetSimApp::Loaded(state) => state.view().map(|msg| Message::LoadedMessage(msg)),
		}
	}
	/* fn scale_factor(&self) -> f64 { 1.0 } */
}
