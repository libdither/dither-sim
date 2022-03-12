#![feature(try_blocks)]

#[macro_use]
extern crate serde;
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate derivative;

mod internet;

use internet::{Internet};

fn main() {
	// Check if necessary kernel features are available
	netsim_embed::unshare_user().expect("netsim: User namespaces are not enabled");
	netsim_embed::Namespace::unshare().expect("netsim: network namespaces are not enabled");
	netsim_embed_machine::iface::Iface::new().expect("netsim: tun adapters not supported");
	
	netsim_embed::run(async {
		let mut internet = Internet::new("./target/debug/device");
		let (runtime, _receiver, _sender) = internet.init().await.expect("Failed to initialize network");
		internet.run(runtime).await;
	});
}