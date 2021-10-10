
use futures::StreamExt;
use libp2p::{Multiaddr, NetworkBehaviour, PeerId, Transport, core::{transport::ListenerEvent, upgrade}, floodsub::{self, Floodsub, FloodsubEvent}, identity, mdns::{Mdns, MdnsEvent}, mplex, noise, swarm::{NetworkBehaviourEventProcess, SwarmBuilder, SwarmEvent}, tcp::TokioTcpConfig};
use std::{error::Error, net::Ipv4Addr};
use tokio::{io::{self, AsyncBufReadExt}, sync::mpsc};

use node::{Node, net::NetAction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	env_logger::init();

	// Create a random PeerId
	let id_keys = identity::Keypair::generate_ed25519();
	let peer_id = PeerId::from(id_keys.public());

	let (tx, mut rx) = mpsc::channel(20);
	let node = Node::new(peer_id.to_bytes(), tx);
	let node_action_sender = node.action_sender.clone();
	let join = node.spawn();
	
	println!("Local peer id: {:?}", peer_id);

	// Create a keypair for authenticated encryption of the transport.
	let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
		.into_authentic(&id_keys)
		.expect("Signing libp2p-noise static DH keypair failed.");
	let transport = TokioTcpConfig::new()
		.nodelay(true)
		.upgrade(upgrade::Version::V1)
		.authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
		.multiplex(mplex::MplexConfig::new())
		.boxed();
	
	let listen_addr1: Multiaddr = ("/ip4/0.0.0.0/tcp/".to_owned() + &std::env::args().nth(1).unwrap()).parse().unwrap();

	let mut listener = transport.clone().listen_on(listen_addr1).expect("listener didn't open");

	loop {
		tokio::select! {
			net_action = rx.recv() => {
				if let Some(net_action) = net_action {
					match net_action {
						NetAction::Incoming(connection) => {
							println!("Received Connection from: {:?}", connection);
						}
						_ => {},
					}
				} else { break; }
			}
			peer_event = listener.next() => {
				match peer_event.unwrap()? {
					ListenerEvent::NewAddress(listen_addr) => {
						println!("Listening on {:?}", listen_addr)
					}
					_ => {},
				}
			}
		}
	}

	if let Ok(node) = join.await {
		println!("Node: {:?} Exited", node.net_addr);
	} else {
		println!("Node Exited with Error");
	}
		
		
	

	Ok(())
}