#![feature(try_blocks)]

#[macro_use]
extern crate serde;
extern crate log;
#[macro_use]
extern crate thiserror;
/* #[macro_use]
extern crate derivative; */

mod internet;
pub use internet::*;

/// This function must be run in a single-threaded program because it calls unshare(2)
pub fn init() {
	netsim_embed::unshare_user().expect("netsim: User namespaces are not enabled");
	netsim_embed::Namespace::unshare().expect("netsim: network namespaces are not enabled");
	netsim_embed_machine::iface::Iface::new().expect("netsim: tun adapters not supported");
}