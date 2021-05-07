#[macro_use] extern crate serde;

use std::{fs, io::BufReader};

use dbr_sim::{self, DEFAULT_CACHE_FILE, NetSim, Node};
use rand::{rngs::SmallRng, SeedableRng};
mod cli;

fn main() -> anyhow::Result<()> {
	env_logger::init();
	println!("Hello, Network!");
	let _ = fs::create_dir_all("target/images");

	let rng = &mut SmallRng::seed_from_u64(0);
	// Try and read cache file, else gen new network
	let mut internet = if let Ok(cache_reader) = fs::File::open(DEFAULT_CACHE_FILE).map(|f| BufReader::new(f)) {
		println!("Loaded Cached Network: {}", DEFAULT_CACHE_FILE);
		NetSim::<Node>::from_reader(cache_reader)?
	} else {
		NetSim::<Node>::new()
	};

	cli::run(&mut internet, rng)?;
	Ok(())
}