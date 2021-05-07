#![feature(drain_filter)]
#![feature(backtrace)]
#![feature(try_blocks)]

#[macro_use]
extern crate serde;
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate derivative;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate slotmap;

pub mod internet;
pub use internet::NetSim;
pub mod node;
pub use node::Node;
#[cfg(feature = "plot")]
pub mod plot;

pub const DEFAULT_CACHE_FILE: &str = "./target/net.cache";

/* fn main() -> anyhow::Result<()> {
	env_logger::init();
	println!("Hello, Network!");
	let _ = fs::create_dir_all("target/images");

	let rng = &mut rand::rngs::SmallRng::seed_from_u64(0);
	// Try and read cache file, else gen new network
	let internet = if let Ok(cache_reader) = fs::File::open(CACHE_FILE).map(|f| BufReader::new(f)) {
		println!("Loaded Cached Network: {}", CACHE_FILE);
		NetSim::<Node>::from_reader(cache_reader)?
	} else {
		NetSim::<Node>::new()
	};

	//cli::run(&mut internet, rng)
	NetSimApp::run(Settings::with_flags(NetSimAppSettings { net_sim: internet }))?;
	Ok(())
} */
