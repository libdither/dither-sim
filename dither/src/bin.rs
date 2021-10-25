
use libdither::{DitherCore, Multiaddr};


#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();
	
	let listen_addr: Multiaddr = ("/ip4/0.0.0.0/tcp/".to_owned() + &std::env::args().nth(1).unwrap()).parse().unwrap();
	let core = DitherCore::init(listen_addr)?;
	core.run().await?;
	Ok(())
}