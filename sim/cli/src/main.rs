#[macro_use] extern crate serde;


use sim::{self, DEFAULT_CACHE_FILE, Internet, Node};
use rand::{rngs::SmallRng, SeedableRng};
mod cli;

fn main() -> anyhow::Result<()> {
	env_logger::init();
	println!("Hello, Network!");
	let _ = std::fs::create_dir_all("target/images");

	let rng = &mut SmallRng::seed_from_u64(0);
	// Try and read cache file, else gen new network
	let mut internet = Internet::new();
	if let Err(err) = internet.load(DEFAULT_CACHE_FILE) {
		println!("Failed to load internet cache file: {:?}", err);
	}

	cli::run(&mut internet, rng)?;
	Ok(())
}