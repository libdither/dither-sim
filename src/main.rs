use std::{fs, io::BufReader};

use sim::{DEFAULT_CACHE_FILE, NetSim, Node};
use iced::{Application, Settings};
use rand::{rngs::SmallRng, SeedableRng};

use crate::gui::{NetSimApp, NetSimAppSettings};
mod gui;
mod tabs;

fn main() -> anyhow::Result<()> {
	env_logger::init();
	println!("Hello, Network!");
	let _ = fs::create_dir_all("target/images");

	let _rng = &mut SmallRng::seed_from_u64(0);
	// Try and read cache file, else gen new network
	let internet = if let Ok(cache_reader) = fs::File::open(DEFAULT_CACHE_FILE).map(|f| BufReader::new(f)) {
		println!("Loaded Cached Network: {}", DEFAULT_CACHE_FILE);
		NetSim::<Node>::from_reader(cache_reader)?
	} else {
		NetSim::<Node>::new()
	};

	//cli::run(&mut internet, rng)
	NetSimApp::run(Settings::with_flags(NetSimAppSettings { net_sim: internet }))?;
	Ok(())
}