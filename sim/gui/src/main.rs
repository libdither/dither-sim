#![feature(try_blocks)]

#[macro_use]
extern crate derivative;
#[macro_use]
extern crate slotmap;


use sim::{Internet, DEFAULT_CACHE_FILE};
use iced::{Application, Settings, window};
use rand::{rngs::SmallRng, SeedableRng};

use crate::gui::{NetSimApp, NetSimAppSettings};
mod tabs;
mod network_map;
mod gui;

fn main() -> anyhow::Result<()> {
	env_logger::init();
	println!("Hello, Network!");
	let _ = std::fs::create_dir_all("target/images");

	let _rng = &mut SmallRng::seed_from_u64(0);

	let mut internet = Internet::new();
	if let Err(err) = internet.load(DEFAULT_CACHE_FILE) {
		log::warn!("Failed to load Default Cache File: {:?}", err);
	}

	let mut settings = Settings::with_flags(NetSimAppSettings {net_sim: internet});
	settings.window = window::Settings {
		resizable: false,
		..Default::default()
	};

	NetSimApp::run(settings)?;

	Ok(())
}