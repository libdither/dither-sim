#![allow(dead_code)]
#![feature(try_blocks)]
#![feature(io_error_more)]

use std::net::SocketAddr;

use futures::{StreamExt, channel::mpsc, SinkExt, FutureExt};
use async_std::{net::{TcpListener, TcpStream}, io::ErrorKind};
use rkyv::Archived;

use node::net::Network;
pub use node::{self, Node, NodeAction, net::{NetAction, ConnectionResponse}};

pub mod commands;
pub use commands::{DitherCommand, DitherEvent};

pub struct DitherCore {
	stored_node: Option<Node<DitherNet>>,
	node_network_receiver: mpsc::Receiver<NetAction<DitherNet>>,
	node_action_sender: mpsc::Sender<NodeAction<DitherNet>>,

	listen_addr: Address,
	event_sender: mpsc::Sender<DitherEvent>,
}

#[derive(Debug, Clone)]
pub struct DitherNet;
impl Network for DitherNet {
	type Address = SocketAddr;
	type ArchivedAddress = Archived<Self::Address>;
	type Conn = TcpStream;
}

pub type Address = <DitherNet as Network>::Address;

impl DitherCore {
	pub fn init(listen_addr: Address) -> anyhow::Result<(DitherCore, mpsc::Receiver<DitherEvent>)> {
		let (tx, node_network_receiver) = mpsc::channel(20);
		let node = Node::<DitherNet>::new(Node::<DitherNet>::gen_id(), tx);
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
		
	
		
		let listener = TcpListener::bind(self.listen_addr).await?;
		let mut incoming = listener.incoming();
	
		let node_network_receiver = &mut self.node_network_receiver;
		loop {
			futures::select! {
				dither_command = dither_command_receiver.next()  => {
					let result: anyhow::Result<()> = try {
						match dither_command.ok_or(anyhow::anyhow!("failed to receive dither command"))? {
							DitherCommand::GetNodeInfo => self.node_action_sender.try_send(NodeAction::NetAction(NetAction::GetNodeInfo))?,
							DitherCommand::Bootstrap(node_id, addr) => self.node_action_sender.try_send(NodeAction::Bootstrap(node_id, addr))?,
						}
					};
					if let Err(err) = result { println!("Dither Command error: {}", err) }
				}
				net_action = node_network_receiver.next() => { // Listen for net actions from Dither Node's Network API
					if let Some(net_action) = net_action {
						let result: anyhow::Result<()> = try {
							match net_action {
								NetAction::Incoming(addr, connection) => {
									println!("Received Connection from {:?}: {:?}", addr, connection);
								}
								NetAction::Connect(addr) => {
									// Connect to remote
									let resp = match TcpStream::connect(addr.clone()).await {
										Ok(conn) => ConnectionResponse::Established(conn),
										Err(err) => match err.kind() {
											ErrorKind::HostUnreachable => ConnectionResponse::NotFound,
											_ => ConnectionResponse::Error(format!("{}", err)),
										}
									};
									self.node_action_sender.send(NodeAction::NetAction(NetAction::ConnectResponse(addr, resp))).await?;
								}
								NetAction::NodeInfo(node_info) => {
									self.event_sender.send(DitherEvent::NodeInfo(node_info)).await?;
								}
								_ => {},
							}
						};
						if let Err(err) = result { println!("NetAction error: {err}") }
					} else { break; }
				}
				tcp_stream = incoming.next().fuse() => { // Listen for incoming connections
					if let Some(Ok(tcp_stream)) = tcp_stream {
						println!("Received new connection: {:?}", tcp_stream);
						let addr = tcp_stream.peer_addr().unwrap();
						if let Err(err) = self.node_action_sender.send(NodeAction::NetAction(NetAction::Incoming(addr, tcp_stream))).await {
							log::error!("Failed to send new Connection to Node: {}", err);
						}
					}
				}
				complete => break,
			}
		}
		
		let node = join.await;
		self.stored_node = Some(node);

		Ok(self)
	}
}

