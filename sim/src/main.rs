#![feature(try_blocks)]

#[macro_use]
extern crate serde;
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate derivative;

mod internet;

use futures::channel::mpsc;

use internet::{Internet, InternetAction, InternetEvent, InternetError};


fn main() {
	// Create channel for sender thread
	let (action_sender, action_receiver) = mpsc::channel::<InternetAction>(20);
	let (event_sender, event_receiver) = mpsc::channel::<InternetEvent>(20);

	// Check if necessary kernel features are available
	netsim_embed::unshare_user().expect("netsim: User namespaces are not enabled");
	netsim_embed::Namespace::unshare().expect("netsim: network namespaces are not enabled");
	netsim_embed_machine::iface::Iface::new().expect("netsim: tun adapters not supported");
	
	let internet = Internet::new();
	netsim_embed::run(internet.run(event_sender, action_receiver));

	/* let action_parsing_thread = thread::spawn(|| {
		let stdin = std::io::stdin();
		let mut input = String::new();
		while let Ok(_) = stdin.read_line(&mut input) {
			if let Ok(action) = InternetAction::from_str(&input) {
				action_sender.try_send(command)?.expect("Command Sender should be open");
			} else {
				println!("Invalid InternetAction (must be RON-formatted string): {:?}", input);
			}
			input.clear();
		}
		()
	});

	let event_print_thread = thread::spawn(|| {
		while let Some(event) = event_receiver.recv().await {
			println!("{}", event); // Print to stdout
		}
	});

	let mut internet = Internet::new();

	let stdin = std::io::stdin();
	let mut input = String::new();

	while let Ok(_) = stdin.read_line(&mut input) {
		if let Ok(action) = InternetAction::from_str(&input) {
			let potential_event = internet.parse(action);
			/// Print any outgoing events
			if let Some(event) = potential_event { println!("{}", event); }
		} else {
			println!("Invalid InternetAction (must be RON-formatted string): {:?}", input);
		}
		input.clear();
	} */
}