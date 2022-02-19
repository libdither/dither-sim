#![allow(dead_code)]
#![feature(try_blocks)]

pub use commands::{DitherCommand, DitherEvent};
use futures::StreamExt;
use libp2p::{Transport, core::transport::{ListenerEvent, upgrade}, identity, mplex, noise, tcp::TokioTcpConfig};

use tokio::{sync::mpsc};

pub use node::{self, Node, NodeAction, net::NetAction};

pub use libp2p::{Multiaddr, PeerId};

pub mod commands;

pub struct DitherCore {
	keypair: identity::Keypair,
	peer_id: PeerId,
	stored_node: Option<Node>,
	node_network_receiver: mpsc::Receiver<NetAction>,
	node_action_sender: mpsc::Sender<NodeAction>,

	listen_addr: Multiaddr,
	event_sender: mpsc::Sender<DitherEvent>,
}

impl DitherCore {
	pub fn init(listen_addr: Multiaddr) -> anyhow::Result<(DitherCore, mpsc::Receiver<DitherEvent>)> {
		let keypair = identity::Keypair::generate_ed25519();
		let peer_id = PeerId::from(keypair.public());

		let (tx, node_network_receiver) = mpsc::channel(20);
		let node = Node::new(peer_id.to_bytes().into(), tx);
		let node_action_sender = node.action_sender.clone();
		let (event_sender, dither_event_receiver) = mpsc::channel(20);
		let core = DitherCore {
			keypair,
			peer_id,
			stored_node: Some(node),
			node_network_receiver,
			node_action_sender,
			listen_addr,
			event_sender,
		};

		Ok((core, dither_event_receiver))
	}
	pub async fn run(mut self, mut dither_command_receiver: mpsc::Receiver<DitherCommand>) -> anyhow::Result<Self> {
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
				dither_command = dither_command_receiver.recv() => {
					let result: anyhow::Result<()> = try {
						match dither_command.ok_or(anyhow::anyhow!("failed to receive dither command"))? {
							DitherCommand::GetNodeInfo => self.node_action_sender.try_send(NodeAction::NetAction(NetAction::GetNodeInfo))?,
						}
					};
					if let Err(err) = result { println!("Dither Command error: {}", err) }
				}
				net_action = node_network_receiver.recv() => { // Listen for net actions from Dither Node's Network API
					if let Some(net_action) = net_action {
						match net_action {
							NetAction::Incoming(connection) => {
								println!("Received Connection from: {:?}", connection);
							}
							NetAction::NodeInfo(node_info) => {
								self.event_sender.try_send(DitherEvent::NodeInfo(node_info)).expect("failed to send dither event");
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

