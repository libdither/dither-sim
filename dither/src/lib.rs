#![allow(dead_code)]

use futures::StreamExt;
use libp2p::{Transport, core::transport::{ListenerEvent, upgrade}, identity, mplex, noise, tcp::TokioTcpConfig};

use tokio::{sync::mpsc};

pub use node::{self, Node, NodeAction, net::NetAction};

pub use libp2p::{Multiaddr, PeerId};

pub mod commands;

pub struct DitherCore {
	keypair: identity::Keypair,
	pub peer_id: PeerId,
	stored_node: Option<Node>,
	node_network_receiver: mpsc::Receiver<NetAction>,
	pub node_action_sender: mpsc::Sender<NodeAction>,

	pub listen_addr: Multiaddr,
}

impl DitherCore {
	pub fn init(listen_addr: Multiaddr) -> anyhow::Result<DitherCore> {
		let keypair = identity::Keypair::generate_ed25519();
		let peer_id = PeerId::from(keypair.public());

		let (tx, node_network_receiver) = mpsc::channel(20);
		let node = Node::new(peer_id.to_bytes(), tx);
		let node_action_sender = node.action_sender.clone();
		let core = DitherCore {
			keypair,
			peer_id,
			stored_node: Some(node),
			node_network_receiver,
			node_action_sender,
			listen_addr,
		};

		Ok(core)
	}
	pub async fn run(mut self) -> anyhow::Result<Self> {
		let join = if let Some(node) = self.stored_node.take() {
			node.spawn()
		} else { return Ok(self); };
		
		println!("Local peer id: {:?}", self.peer_id);
	
		// Create a keypair for authenticated encryption of the transport.
		let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
			.into_authentic(&self.keypair)
			.expect("Signing libp2p-noise static DH keypair failed.");
		let transport = TokioTcpConfig::new()
			.nodelay(true)
			.upgrade(upgrade::Version::V1)
			.authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
			.multiplex(mplex::MplexConfig::new())
			.boxed();
		
	
		let mut listener = transport.clone().listen_on(self.listen_addr.clone()).expect("listener didn't open");
	
		let node_network_receiver = &mut self.node_network_receiver;
		loop {
			tokio::select! {
				net_action = node_network_receiver.recv() => { // Listen for network actions from Node impl
					if let Some(net_action) = net_action {
						match net_action {
							NetAction::Incoming(connection) => {
								println!("Received Connection from: {:?}", connection);
							}
							_ => {},
						}
					} else { break; }
				}
				peer_event = listener.next() => { // Listen for peer events from libp2p
					match peer_event.unwrap()? {
						ListenerEvent::NewAddress(listen_addr) => {
							println!("Listening on {:?}", listen_addr)
						}
						_ => {},
					}
				}
			}
		}
		
		let node = join.await.expect("Node should not error");
		self.stored_node = Some(node);

		Ok(self)
	}
}

