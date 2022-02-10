
use std::pin::Pin;

use iced_futures::subscription::Recipe;
use sim::{Internet, InternetAction, InternetError, InternetEvent};
use futures::{StreamExt, channel::mpsc};
use async_std::task::{self, JoinHandle};

// NO TOUCHIE: iced subscriptions system is really finicky, don't mess with it.
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
						log::debug!("Initializing Network Subscription from: {:?}", path);
						match if let Some(path) = path {
							Internet::load(&path)
						} else { Ok(Internet::new("./target/debug/device")) } {
							Ok(mut internet) =>{
								match internet.init().await {
									Ok((runtime, receiver, sender)) => {
										let join = task::spawn(internet.run(runtime));
										Some((
											Event::Init(sender),
											State::Running(receiver, join)
										))
									}
									Err(err) =>  Some((
										Event::Error(err),
										State::Finished,
									)),
								}
							},
							Err(err) => Some((
								Event::Error(err),
								State::Finished,
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