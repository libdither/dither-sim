#![feature(try_blocks)]

#[macro_use]
extern crate derivative;
#[macro_use]
extern crate slotmap;

use iced::{Application, Settings, window};
use crate::gui::{NetSimApp, NetSimAppSettings};
mod tabs;
mod network_map;
mod subscription;
mod gui;

fn main() -> anyhow::Result<()> {
	env_logger::init();
	println!("Hello, Network!");

	let mut settings = Settings::with_flags(NetSimAppSettings {});
	settings.window = window::Settings {
		resizable: false,
		..Default::default()
	};
	NetSimApp::run(settings)?;

	Ok(())
}