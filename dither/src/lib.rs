#![allow(dead_code)]
#![feature(try_blocks)]

use std::{convert::TryFrom, net::Ipv4Addr};

pub use commands::{DitherCommand, DitherEvent};
use futures::{StreamExt, channel::mpsc};

use async_std::{net::TcpStream};

use node::net::Network;
pub use node::{self, Node, NodeAction, net::NetAction};

pub mod commands;

pub struct DitherCore {
	stored_node: Option<Node<Self>>,
	node_network_receiver: mpsc::Receiver<NetAction<Self>>,
	node_action_sender: mpsc::Sender<NodeAction<Self>>,

	listen_addr: Multiaddr,
	event_sender: mpsc::Sender<DitherEvent>,
}
impl Network for DitherCore {
	type Address = (Ipv4Addr, u16);
	type Connection = TcpStream;
}


impl DitherCore {
	pub fn init(listen_addr: Multiaddr) -> anyhow::Result<(DitherCore, mpsc::Receiver<DitherEvent>)> {
		let (tx, node_network_receiver) = mpsc::channel(20);
		let node = Node::new(peer_id.to_bytes().into(), tx);
		let node_action_sender = node.action_sender.clone();
		let (event_sender, dither_event_receiver) = mpsc::channel(20);
		let core = DitherCore {
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
	
		let mut listener = TcpListener::bind(self.listen_addr).await?.incoming();
	
		let node_network_receiver = &mut self.node_network_receiver;
		loop {
			futures::select! {
				dither_command = dither_command_receiver.recv() => {
					let result: anyhow::Result<()> = try {
						match dither_command.ok_or(anyhow::anyhow!("failed to receive dither command"))? {
							DitherCommand::GetNodeInfo => self.node_action_sender.try_send(NodeAction::NetAction(NetAction::GetNodeInfo))?,
							DitherCommand::Bootstrap(node_id, addr) => self.node_action_sender.try_send(NodeAction::Bootstrap(node_id, addr))?,
						}
					};
					if let Err(err) = result { println!("Dither Command error: {}", err) }
				}
				net_action = node_network_receiver.recv() => { // Listen for net actions from Dither Node's Network API
					if let Some(net_action) = net_action {
						let result: anyhow::Result<()> = try {
							match net_action {
								NetAction::Incoming(connection) => {
									println!("Received Connection from: {:?}", connection);
								}
								NetAction::Connect(addr) => {
									let multiaddr = Multiaddr::try_from(addr.0)?;
									println!("Dialing: {}", multiaddr);
									let (peer, _stream) = transport.clone().dial(multiaddr)?.await?;
									println!("Established connection with: {:?}", peer);
								}
								NetAction::NodeInfo(node_info) => {
									self.event_sender.try_send(DitherEvent::NodeInfo(node_info)).expect("failed to send dither event");
								}
								_ => {},
							}
						};
						if let Err(err) = result { println!("NetAction error: {err}") }
					} else { break; }
				}
				tcp_stream = listener.next() => { // Listen for incoming connections
					if let Ok(Some(tcp_stream)) = tcp_stream {
						println!("Received new connection: {:?}", tcp_stream);
						let address = tcp_stream.local_addr();
						self.node_action_sender(NodeAction::NetAction(NetAction::Incoming()))
					}
				}
				complete => break,
			}
		}
		
		let node = join.await.expect("Node should not error");
		self.stored_node = Some(node);

		Ok(self)
	}
}

