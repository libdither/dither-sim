#![allow(dead_code)]
#![feature(try_blocks)]
#![feature(io_error_more)]

use std::net::SocketAddr;

use futures::{StreamExt, channel::mpsc, SinkExt, FutureExt};
use async_std::{io::ErrorKind, net::{TcpListener, TcpStream}, task::{self, JoinHandle}};
use rkyv::Archived;

use node::net::Network;
pub use node::{self, Node, NodeAction, net::{NetAction, NetEvent, UserAction, UserEvent, ConnectionResponse, Connection}};

pub mod commands;
pub use commands::{DitherCommand, DitherEvent};

pub struct DitherCore {
	stored_node: Option<Node<DitherNet>>,
	node_network_receiver: mpsc::Receiver<NetAction<DitherNet>>,
	node_network_sender: mpsc::Sender<NetAction<DitherNet>>,
	listen_addr: Address,
	event_sender: mpsc::Sender<DitherEvent>,
}

#[derive(Debug, Clone)]
pub struct DitherNet;
impl Network for DitherNet {
	type Address = SocketAddr;
	type ArchivedAddress = Archived<Self::Address>;
	type Read = TcpStream;
	type Write = TcpStream;
}

pub type Address = <DitherNet as Network>::Address;

impl DitherCore {
	pub fn init(listen_addr: Address) -> anyhow::Result<(DitherCore, mpsc::Receiver<DitherEvent>)> {
		let (node_network_sender, node_network_receiver) = mpsc::channel(20);
		let node = Node::<DitherNet>::new(Node::<DitherNet>::gen_id());
		
		let (event_sender, dither_event_receiver) = mpsc::channel(20);
		let core = DitherCore {
			stored_node: Some(node),
			node_network_receiver,
			node_network_sender,
			listen_addr,
			event_sender,
		};

		Ok((core, dither_event_receiver))
	}
	pub async fn run(mut self, mut dither_command_receiver: mpsc::Receiver<DitherCommand>) -> anyhow::Result<Self> {
		let (node_join, mut node_action_sender) = if let Some(node) = self.stored_node {
			node.spawn(self.node_network_sender.clone())
		} else { Err(anyhow::anyhow!("No stored node"))? };
		
		let listener = TcpListener::bind(self.listen_addr).await?;
		let mut incoming = listener.incoming();
	
		let node_network_receiver = &mut self.node_network_receiver;
		loop {
			futures::select! {
				dither_command = dither_command_receiver.next()  => {
					let result: anyhow::Result<()> = try {
						match dither_command.ok_or(anyhow::anyhow!("failed to receive dither command"))? {
							DitherCommand::GetNodeInfo => node_action_sender.try_send(NodeAction::NetEvent(NetEvent::UserAction(UserAction::GetNodeInfo)))?,
							DitherCommand::Bootstrap(node_id, addr) => node_action_sender.try_send(NodeAction::Bootstrap(node_id, addr))?,
						}
					};
					if let Err(err) = result { println!("Dither Command error: {}", err) }
				}
				net_action = node_network_receiver.next() => { // Listen for net actions from Dither Node's Network API
					if let Some(net_action) = net_action {
						let result: anyhow::Result<()> = try {
							match net_action {
								NetAction::Connect(addr) => {
									// Connect to remote
									let mut action_sender = node_action_sender.clone();
									let _ = task::spawn(async move {
										let conn_resp = match TcpStream::connect(addr.clone()).await {
											Ok(conn) => ConnectionResponse::Established(Connection { addr, read: conn.clone(), write: conn }),
											Err(err) => match err.kind() {
												ErrorKind::HostUnreachable => ConnectionResponse::NotFound(addr),
												_ => ConnectionResponse::Error(addr, format!("{}", err)),
											}
										};
										action_sender.send(NodeAction::NetEvent(NetEvent::ConnectResponse(conn_resp))).await.unwrap();
									});
								}
								NetAction::UserEvent(user_event) => {
									match user_event {
										UserEvent::NodeInfo(node_info) => {
											self.event_sender.send(DitherEvent::NodeInfo(node_info)).await?;
										}
									}
									
								}
							}
						};
						if let Err(err) = result { println!("NetAction error: {err}") }
					} else { break; }
				}
				tcp_stream = incoming.next().fuse() => { // Listen for incoming connections
					if let Some(Ok(tcp_stream)) = tcp_stream {
						println!("Received new connection: {:?}", tcp_stream);
						let addr = tcp_stream.peer_addr().unwrap();
						let conn = Connection { addr, read: tcp_stream.clone(), write: tcp_stream };
						if let Err(err) = node_action_sender.send(NodeAction::NetEvent(NetEvent::Incoming(conn))).await {
							log::error!("Failed to send new Connection to Node: {}", err);
						}
					}
				}
				complete => break,
			}
		}
		
		self.stored_node = Some(node_join.await);

		Ok(self)
	}
}

