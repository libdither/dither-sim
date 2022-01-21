
use libdither::{DitherCore, Multiaddr};
use tokio::sync::mpsc;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();
	
	let listen_addr: Multiaddr = ("/ip4/0.0.0.0/tcp/".to_owned() + &std::env::args().nth(1).unwrap()).parse().unwrap();
	let (core, _event_receiver) = DitherCore::init(listen_addr)?;
	let (_command_sender, command_receiver) = mpsc::channel(20);
	core.run(command_receiver).await?;

	Ok(())
}