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

use internet::{Internet, InternetAction, InternetEvent, };


fn main() {
	// Check if necessary kernel features are available
	netsim_embed::unshare_user().expect("netsim: User namespaces are not enabled");
	netsim_embed::Namespace::unshare().expect("netsim: network namespaces are not enabled");
	netsim_embed_machine::iface::Iface::new().expect("netsim: tun adapters not supported");
	
	netsim_embed::run(async {
		let (internet, _event_receiver, _action_sender) = Internet::new();
		internet.run().await;
	});
}