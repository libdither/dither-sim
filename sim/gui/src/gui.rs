#![allow(dead_code)]

pub use iced::Settings;
use iced::{executor, Command, Subscription, pure::{Application, Element}};

use iced_native::command::Action;
use sim::{InternetAction, InternetError};

use crate::subscription::{self, Event};

pub mod loaded;
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
	SubscriptionEvent(subscription::Event),
	InternetAction(InternetAction),
}

impl Application for NetSimApp {
	type Executor = executor::Default;
	type Message = Message;
	type Flags = NetSimAppSettings;

	fn new(_flags: NetSimAppSettings) -> (Self, Command<Self::Message>) {
		(
			NetSimApp::Loading(loading::State { text_input_string: "./target/internet.bin".into(), valid_file: true, ..Default::default() }),
			Command::single(Action::Future(Box::pin(async { Self::Message::LoadingMessage(loading::Message::TriggerLoad) }))),
		)
	}

	fn title(&self) -> String {
		String::from("Dither Network Simulation")
	}

	fn subscription(&self) -> Subscription<Message> {
		match self {
			NetSimApp::Loading(state) => {
				if let Some(recipe) = &state.currently_loading_recipe {
					Subscription::from_recipe(recipe.clone()).map(Message::SubscriptionEvent)
				} else {
					Subscription::none()
				}
			}
			// State is changed to Loading once Init Event triggered from Subscription
			NetSimApp::Loaded(state) => {
				Subscription::from_recipe(state.internet_recipe.clone()).map(Message::SubscriptionEvent)
			},
		}
	}

	fn update(&mut self, message: Message) -> Command<Self::Message> {
		match self {
			NetSimApp::Loading(state) => {
				log::trace!("[Loading] Received Message: {:?}", message);
				match message {
					Message::LoadingMessage(message) => {
						if let Some(message) = state.process(message) {
							self.update(message);
						}
					}
					Message::LoadInternet => {
						println!("Loading Internet: {:?}", state.currently_loading_recipe);
					}
					Message::SubscriptionEvent(event) => {
						match event {
							Event::Init(sender) => {
								*self = NetSimApp::Loaded(loaded::State::new(
									sender,
									state.currently_loading_recipe.take().expect("There should be a recipe if internet is loaded")
								));
							}
							Event::Error(error) => {
								match error {
									InternetError::Other(err) => log::error!("InternetError: {}", err),
									_ => log::error!("Received InternetError {:?} in Loading state, but error doesn't apply", error),
								}
							}
							_ => log::error!("Received subscription event {:?} in Loading state, state should have already switched to Loaded", event)
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
							self.update(message);
						}
					}
					Message::InternetAction(action) => {
						if let Err(err) = state.internet_action.try_send(action) {
							log::error!("Couldn't send action to simulation: {:?}", err);
						}
					}
					Message::SubscriptionEvent(event) => {
						match event {
							Event::Event(internet_event) => {
								state.process(loaded::Message::InternetEvent(internet_event));
							}
							Event::Error(err) => log::error!("Internet Sim errored: {:?}", err),
							Event::Closed => {
								log::info!("Internet Sim closed");
								*self = NetSimApp::Loading(loading::State::default());
							},
							_ => log::error!("Received subscription event {:?} in Loaded state, but event only applies to Loading state", event),
						}
					}
					_ => log::error!("Received Message: {:?} but inapplicable to loaded state", message)
				}
			}
		}
		Command::none()
	}

	fn view(&self) -> Element<Message> {
		match self {
			NetSimApp::Loading(state) => state.view().map(|msg| Message::LoadingMessage(msg)),
			NetSimApp::Loaded(state) => state.view().map(|msg| Message::LoadedMessage(msg)),
		}
	}
	/* fn scale_factor(&self) -> f64 { 1.0 } */
}
