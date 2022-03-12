//! This is the remote module, It manages actions too and from a remote node
//!

use crate::{NodeAction, NodeID, RouteCoord, net::{Connection, Network}, packet::{PacketRead, PacketWrite, AckNodePacket, ArchivedAckNodePacket, ArchivedNodePacket, NodePacket}, ping::PingTracker, session::Session};

use async_std::task::{self, JoinHandle};
use futures::{
	channel::mpsc::{self, Receiver, Sender},
	FutureExt, SinkExt, StreamExt,
};

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Infallible, Serialize, option::ArchivedOption};
use rkyv_codec::{RkyvCodecError};

// Info stored by the node for the current session
#[derive(Debug)]
pub struct SessionInfo {
	pub total_remotes: usize,
}

/// Actions received from main thread.
#[derive(Debug)]
pub enum RemoteAction<Net: Network> {
	/// Bootstrap off of Net::Address
	Bootstrap(Net::Address),
	/// Handle new Connection
	HandleConnection(Connection<Net>),
	/// Query Route Coord from Route Coord Lookup (see NetAction)
	RouteCoordQuery(RouteCoord),

	/// Used by the main node to notify remote threads of any updated info
	UpdateInfo(SessionInfo),
}

#[derive(Error, Debug)]
pub enum RemoteError {
	#[error("No active session")]
	SessionInactive,
	#[error("Received Acknowledgement even though there are no pending handshake requests")]
	NoPendingHandshake,
	#[error("Packet Codec Error")]
	CodecError(#[from] RkyvCodecError),
}

#[derive(Debug, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct DirectRemote<Net: Network> {
	addr: Net::Address,
	route_coord: RouteCoord,
	remote_count: usize,
	considered_active: bool,

	ping_tracker: PingTracker,
}
impl<Net: Network> DirectRemote<Net> {
	pub fn new(addr: Net::Address) -> Self {
		Self {
			addr,
			route_coord: RouteCoord::default(),
			remote_count: 0,
			considered_active: false,
			ping_tracker: PingTracker::new(),
		}
	}
	// Send packet as acknowledgement
	async fn send_ack(&mut self, writer: &mut PacketWrite<Net>, packet_id: u16, packet: &NodePacket<Net>) -> Result<(), RemoteError> {
		let should_ack = !self.ping_tracker.is_stable();
		let packet = AckNodePacket {
			packet,
			packet_id: self.ping_tracker.checkout_unique_id(),
			should_ack,
			acknowledging: Some(packet_id),
		};
		Ok(writer.write_packet(&packet).await?)
	}
	// Send packet
	async fn send_packet(&mut self, writer: &mut PacketWrite<Net>, packet: &NodePacket<Net>, need_ack: bool) -> Result<(), RemoteError> {
		let packet = AckNodePacket {
			packet,
			packet_id: self.ping_tracker.checkout_unique_id(),
			should_ack: need_ack && !self.ping_tracker.is_stable(),
			acknowledging: None,
		};
		Ok(writer.write_packet(&packet).await?)
	}

	async fn handle_connection(
		&mut self,
		self_node_id: NodeID,
		mut action_receiver: Receiver<RemoteAction<Net>>,
		mut reader: PacketRead<Net>,
		mut writer: PacketWrite<Net>,
		mut node_action: Sender<NodeAction<Net>>,
		address: Net::Address,
		mut session_info: SessionInfo,
	) {
		if self.addr != address {
			log::info!("Remote {} changed IP from {} to {}", self_node_id, self.addr, address);
			self.addr = address;
		}
		loop {
			futures::select! {
				// Receive Actions
				action = action_receiver.next() => {
					if let Some(action) = action {
						match action {
							RemoteAction::HandleConnection(connection) => {
								let (addr, reader_new, writer_new) = NodePacket::create_codec(connection);
								reader = reader_new; writer = writer_new;
								log::info!("Remote {} switched connection to: {}", self_node_id, addr);
							},
							RemoteAction::UpdateInfo(info) => session_info = info,
							_ => log::error!("Unsupported Remote Action in inactive state: {:?}", action),
						}
					}
				}
				// Receive Node Packets
				packet = reader.read_packet().fuse() => {
					let ArchivedAckNodePacket { packet, packet_id, should_ack, acknowledging } = packet.unwrap();
					// Register acknowledgement
					if let ArchivedOption::Some(unique_id) = acknowledging { self.ping_tracker.return_unique_id(*unique_id); }

					match packet {
						// If receive Bootstrap Request, send Info packet
						ArchivedNodePacket::Bootstrap { requester } => {
							self.send_ack(&mut writer, *packet_id, &NodePacket::Info {
								route_coord: self.route_coord,
								active_peers: session_info.total_remotes,
							});
						},
						ArchivedNodePacket::Info { route_coord, active_peers } => {
							if *should_ack { self.send_ack(&mut writer, *packet_id, &NodePacket::Ack).await; }
						},
						ArchivedNodePacket::RequestPeers { nearby } => {
							node_action.send(NodeAction::RequestPeers(self_node_id.clone(), nearby.deserialize(&mut Infallible).unwrap())).await;
						},
						ArchivedNodePacket::WantPeer { requesting, addr } => {
							node_action.send(NodeAction::HandleWantPeer { requesting: requesting.clone(), addr: addr.deserialize(&mut Infallible).unwrap() }).await;
						},
						ArchivedNodePacket::WantPeerResp { prompting_node } => {
							if *should_ack { self.send_ack(&mut writer, *packet_id, &NodePacket::Ack).await; }
						}
						ArchivedNodePacket::Notify { active } => {
							if *should_ack { self.send_ack(&mut writer, *packet_id, &NodePacket::Ack).await; } // TODO: Send back Notify packet instead of Ack
							self.considered_active = *active;
						}
						ArchivedNodePacket::Ack => {
							if *should_ack { self.send_ack(&mut writer, *packet_id, &NodePacket::Ack); }
						},
						
						ArchivedNodePacket::Data(_) => todo!(),
						ArchivedNodePacket::Traversal { destination, session_packet } => todo!(),
						ArchivedNodePacket::Return { packet, origin } => todo!(),
						_ => { log::error!("Found Session packet: {:?}", packet); }
					}
				}
			}
		}
	}
}

#[derive(Debug, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub enum RemoteState<Net: Network> {
	// Node is directly connected
	Direct(DirectRemote<Net>),
	// Node is connected through other nodes
	Traversed { route_coord: RouteCoord },
	Routed { routes: Vec<(RouteCoord, NodeID)> },
}

#[derive(Debug, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct Remote<Net: Network> {
	/// Unique NodeID of the remote
	node_id: NodeID,
	/// State of this node
	state: RemoteState<Net>,
	/// Current encrypted session details
	session: Option<Session<Net>>,
}

impl<Net: Network> Remote<Net> {
	pub fn new_direct(node_id: NodeID, addr: Net::Address) -> Remote<Net> {
		Remote {
			node_id,
			state: RemoteState::Direct(DirectRemote::new(addr)),
			session: None,
		}
	}
	pub fn new_traversed(node_id: NodeID, route_coord: RouteCoord) -> Remote<Net> {
		Remote {
			node_id,
			state: RemoteState::Traversed { route_coord },
			session: None,
		}
	}
	// Run remote action event loop. Consumes itself, should be run on independent thread
	pub fn spawn(
		self,
		node_action: Sender<NodeAction<Net>>,
		connection: Connection<Net>,
		session_info: SessionInfo,
	) -> (JoinHandle<Self>, Sender<RemoteAction<Net>>) {
		let (tx, rx) = mpsc::channel(20);

		let (addr, reader, writer) = NodePacket::create_codec(connection);
		let join = task::spawn(
			self.run(rx, reader, writer, node_action, addr, session_info)
		);
		(join, tx)
	}
	/// Handle active session
	async fn run(
		mut self,
		action_receiver: Receiver<RemoteAction<Net>>,
		reader: PacketRead<Net>,
		writer: PacketWrite<Net>,
		node_action: Sender<NodeAction<Net>>,
		address: Net::Address,
		session_info: SessionInfo,
	) -> Self {
		match &mut self.state {
			// Deal with direct connection
			RemoteState::Direct(direct) => {
				direct.handle_connection(self.node_id.clone(), action_receiver, reader, writer, node_action, address, session_info).await;
			}
			// Deal with a Traversed connection
			RemoteState::Traversed { route_coord } => {
				/* while let Some(action) = action_receiver.next().await {
					node_action.send(NodeAction::SendTraversed())
				} */
			}
			RemoteState::Routed { routes } => {}
		}
		self
	}
}
