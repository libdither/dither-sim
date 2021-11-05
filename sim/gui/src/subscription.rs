
use std::pin::Pin;

use iced_futures::subscription::Recipe;
use sim::{Internet, InternetAction, InternetError, InternetEvent};
use futures::{StreamExt, channel::mpsc};
use async_std::task::{self, JoinHandle};

/* pub fn simulate(path: Option<&str>) -> InternetRecipe {
	let mut internet = Internet::new();
	if let Some(path) = path {
		if let Err(err) = internet.load(path) {
			log::warn!("Failed to load Internet Sim: {:?}", err);
		}
	}
	
	internet.run()
} */

#[derive(Debug, Clone)]
pub struct InternetRecipe {
	pub path: Option<String>,
}

impl<H, E> Recipe<H, E> for InternetRecipe where H: std::hash::Hasher {
	type Output = Event;

	fn hash(&self, state: &mut H) {
		use std::hash::Hash;
		
		std::any::TypeId::of::<Self>().hash(state);
		self.path.hash(state);
	}

	fn stream(self: Box<Self>, _input: Pin<Box<(dyn futures::Stream<Item = E> + std::marker::Send + 'static)>>) -> Pin<Box<(dyn futures::Stream<Item = Self::Output> + std::marker::Send + 'static)>> {
		Box::pin(futures::stream::unfold(
			State::Initialize(self.path),
			move |state| async move {
				match state {
					State::Initialize(path) => {
						let mut internet = Internet::new();
						if let Err(err ) = if let Some(path) = path { internet.load(&path) } else { Ok(()) } {
							Some((
								Event::Error(err),
								State::Finished,
							))
						} else {
							let (sender, rx) = mpsc::channel(20);
							let (tx, receiver) = mpsc::channel(20);
							let join = task::spawn(internet.run(tx, rx));
							
							Some((
								Event::Init(sender),
								State::Running(receiver, join)
							))
						}
					}
					State::Running(mut receiver, join) => {
						let event = receiver.next().await;
						if let Some(event) = event {
							Some((
								Event::Event(event),
								State::Running(receiver, join)
							))
						} else {
							Some((
								Event::Closed,
								State::Finished
							))
						}
						
					}
					State::Finished => {
						let _: () = iced::futures::future::pending().await;
						None
					}
				}
			}
	 	))
	}
}

#[derive(Debug)]
pub enum Event {
	Init(mpsc::Sender<InternetAction>),
	Event(InternetEvent),
	Error(InternetError),
	Closed,
}

enum State {
	Initialize(Option<String>),
	Running(mpsc::Receiver<InternetEvent>, JoinHandle<()>),
	Finished,
}