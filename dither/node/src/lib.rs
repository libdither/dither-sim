//! This is the Node Module, it defines all the behaviours of a Dither Node.
//! It provides a simple API to the internet module containing it.
#![allow(dead_code)]

#![feature(try_blocks)]
#![feature(arbitrary_self_types)]
#![feature(generic_associated_types)]
#![feature(associated_type_bounds)]

#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate derivative;

use async_std::task::{self, JoinHandle};
use futures::{SinkExt, StreamExt, channel::mpsc::{self, Receiver, Sender}};

use std::{collections::{BTreeMap, HashMap}, mem::{self, MaybeUninit}, time::Instant};

use net::{Connection, ConnectionResponse, NetAction, NetEvent, Network, UserAction, UserEvent};
use packet::NodePacket;

pub mod net;
mod packet;
mod remote;
mod types;
mod ping;
mod session;

use remote::{Remote, RemoteAction, RemoteError, SessionInfo};

use slotmap::{SlotMap, new_key_type};

new_key_type! { pub struct RemoteIdx; }


/// Multihash that uniquely identifying a node (represents the Multihash of the node's Public Key)
pub type NodeID = hashdb::Hash;
/// Coordinate that represents a position of a node relative to other nodes in 2D space.
pub type RouteScalar = u64;
/// A location in the network for routing packets
pub type RouteCoord = (i64, i64);

#[derive(Debug)]
pub enum RemoteHandle<Net: Network> {
	Active(JoinHandle<Remote<Net>>, Sender<RemoteAction<Net>>),
	Inactive(Remote<Net>),
}

impl<Net: Network> RemoteHandle<Net> {
	pub fn new(remote: Remote<Net>) -> Self { Self::Inactive(remote) }
	/// Send RemoteAction to remote thread and create if thread doesn't exist.
	async fn activate(&mut self, node_action: &Sender<NodeAction<Net>>, connection: Connection<Net>, session_info: SessionInfo) {
		// Safety: Self is initialized in the next line
		let mut ret = mem::replace(self, unsafe { MaybeUninit::uninit().assume_init() });
		*self = match ret {
			RemoteHandle::Inactive(remote) => {
				// Safety: Self is overwritten
				let (join, sender) = remote.spawn(node_action.clone(), connection, session_info);
				RemoteHandle::Active(join, sender)
			},
			RemoteHandle::Active(_, _) => {
				ret.action(RemoteAction::HandleConnection(connection)).await;
				ret.action(RemoteAction::UpdateInfo(session_info)).await;
				ret
			},
		};
	}
	pub async fn action(&mut self, action: RemoteAction<Net>) -> Result<(), NodeError<Net>> {
		Ok(match self {
			RemoteHandle::Active(_, sender) => {
				sender.send(action).await?
			}
			RemoteHandle::Inactive(_) => Err(RemoteError::SessionInactive)?
		})
	}
}


/// Actions that can be run by an external entity (either the internet implementation or the user)
#[derive(Debug)]
pub enum NodeAction<Net: Network> {
	/// Bootstrap this node onto a specific other network node, starts the self-organization process
	Bootstrap(NodeID, Net::Address),

	/// Connect to network through passed sim::Connection
	/// Initiate Handshake with remote NodeID, net::Addressess and initial packets
	//Connect(net::Connection, NodeID, SessionType, Vec<NodePacket>),

	/// Handle event from Internet
	NetEvent(NetEvent<Net>),
	/// Send Action to network implementation
	NetAction(NetAction<Net>),

	/// Send Arbitrary to Remote
	ForwardPacket(NodeID, NodePacket<Net>),

	/// Request for Another node to ask their peers to connect to me based on peers near me.
	RequestPeers(NodeID, Vec<((i64, i64), u32)>),
	/// Calculate route coordinate using Multilateration
	CalcRouteCoord,
	/// Send info to another node
	SendInfo(NodeID),
	/// Send packet to peer that wants peers
	HandleWantPeer { requesting: NodeID, addr: Net::Address },

	/* /// Send DHT request for Route Coordinate
	RequestRouteCoord(NodeID),
	/// Establish Traversed Session with remote NodeID
	/// Looks up remote node's RouteCoord on DHT and enables Traversed Session
	ConnectTraversed(NodeID, Vec<NodePacket<Net>>),
	/// Establishes Routed session with remote NodeID
	/// Looks up remote node's RouteCoord on DHT and runs CalculateRoute after RouteCoord is received
	/// * `usize`: Number of intermediate nodes to route through
	/// * `f64`: Random intermediate offset (high offset is more anonymous but less efficient, very high offset is random routing strategy)
	ConnectRouted(NodeID, usize),
	/// Send specific packet to node
	SendData(NodeID, Vec<u8>), */
}

#[derive(Error, Debug)]
pub enum NodeError<Net: Network> {
	// Error from Remote Node Thread
	#[error(transparent)]
	RemoteError(#[from] RemoteError),
	#[error("Failed to send message")]
	SendError(#[from] mpsc::SendError),

	// When Accessing Remotes
	#[error("Unknown Node Index: {node_idx:?}")]
	UnknownNodeIndex { node_idx: RemoteIdx },
	#[error("Unknown NodeID: {node_id:?}")]
	UnknownNodeID { node_id: NodeID },
	#[error("Unknown Network Addr: {net_addr:?}")]
	UnknownNetAddr { net_addr: Net::Address },

	#[error("There is no calculated route coordinate for this node")]
	NoCalculatedRouteCoord,
	#[error("There are not enough peers, needed: {required}")]
	InsufficientPeers { required: usize },

	// Catch-all
	#[error(transparent)]
	Other(#[from] anyhow::Error),

}
impl<Net: Network> NodeError<Net> {
	pub fn anyhow(self) -> NodeError<Net> {
		NodeError::Other(anyhow::Error::new(self))
	}
}

#[derive(Debug)]
pub struct Node<Net: Network> {
	/// Unique Identifier for node on the network, known as the Hash of the public key
	pub node_id: NodeID,

	/// Represents what this node is identified as on the network implementation. In real life, there would be multiple of these but for testing purposes there will just be one.
	pub public_addr: Option<Net::Address>,

	/// This node's Distance-Based Routing Coordinates
	pub route_coord: RouteCoord,

	/// Amount of time passed since startup of this node
	pub start_time: Instant,

	/// Hold Info about remote nodes
	remotes: SlotMap<RemoteIdx, RemoteHandle<Net>>,
	/// Map NodeIDs to Remote Node Indicies
	ids: HashMap<NodeID, RemoteIdx>,

	/// Map Addresses to Remote Node Indicies
	//#[serde(skip)]
	addrs: HashMap<Net::Address, RemoteIdx>,

	/// Sorted list of nodes based on how close they are latency-wise
	direct_sorted: BTreeMap<u64, RemoteIdx>, // All nodes that have been tested, sorted by lowest value
}

impl<Net: Network> Node<Net> {
	pub fn gen_id() -> NodeID {
		let random: [u8; 10] = rand::random();
		hashdb::Hash::hash(&random[..])
	}
	/// Create New Node with specific ID
	pub fn new(node_id: NodeID) -> Node<Net> {
		Node {
			node_id,
			public_addr: None,
			route_coord: RouteCoord::default(),
			start_time: Instant::now(),
			remotes: Default::default(),
			ids: Default::default(),
			addrs: Default::default(),
			direct_sorted: Default::default(),
		}
	}

	pub fn remote(&self, node_idx: RemoteIdx) -> Result<&RemoteHandle<Net>, NodeError<Net>> {
		self.remotes
			.get(node_idx)
			.ok_or(NodeError::UnknownNodeIndex { node_idx })
	}
	pub fn remote_mut(&mut self, node_idx: RemoteIdx) -> Result<&mut RemoteHandle<Net>, NodeError<Net>> {
		self.remotes
			.get_mut(node_idx)
			.ok_or(NodeError::UnknownNodeIndex { node_idx })
	}
	pub fn index_by_node_id(&self, node_id: &NodeID) -> Result<RemoteIdx, NodeError<Net>> {
		self.ids
			.get(node_id)
			.cloned()
			.ok_or(NodeError::UnknownNodeID {
				node_id: node_id.clone(),
			})
	}
	pub fn index_by_addr(&self, addr: &Net::Address) -> Result<RemoteIdx, NodeError<Net>> {
		self.addrs
    		.get(addr)
			.cloned()
			.ok_or(NodeError::UnknownNetAddr {
				net_addr: addr.clone(),
			})
	}
	pub fn get_or_new_remote(&mut self, node_id: NodeID, addr: &Net::Address) -> Result<&mut RemoteHandle<Net>, NodeError<Net>> {
		let index = if self.addrs.contains_key(addr) {
			self.index_by_addr(addr)?
		} else {
			let index = self.remotes.insert(RemoteHandle::new(Remote::new_direct(node_id, addr.clone())));
			self.addrs.insert(addr.clone(), index);
			index
		};
		self.remote_mut(index)
	}
	pub fn gen_session_info(&mut self) -> SessionInfo {
		SessionInfo {
			total_remotes: self.remotes.len(),
		}
	}

	pub fn spawn(self, network_action: Sender<NetAction<Net>>) -> (JoinHandle<Node<Net>>, Sender<NodeAction<Net>>) {
		let (action_sender, action_receiver) = mpsc::channel(100);
		let join = task::spawn(self.run(action_sender.clone(), network_action, action_receiver));
		(join, action_sender)
	}
	/// Runs event loop on this object
	async fn run(
		mut self,
		action_sender: Sender<NodeAction<Net>>,
		mut network_action: Sender<NetAction<Net>>,
		mut action_receiver: Receiver<NodeAction<Net>>
	) -> Self {
		let node_action = &mut action_sender.clone();

		while let Some(action) = action_receiver.next().await {
			let node_error: Result<(), NodeError<Net>> = try {
				match action {
					// Initiate Bootstrapping process
					NodeAction::Bootstrap(node_id, addr) => {
						let handle = self.get_or_new_remote(node_id, &addr)?;
						handle.action(RemoteAction::Bootstrap(addr)).await?;
					}
					// Forward Net actions sent by remote
					NodeAction::NetAction(net_action) => network_action.send(net_action).await?,
					// Deal with any Network Events
					NodeAction::NetEvent(net_event) => {
						match net_event {
							// Handle requested connection
							NetEvent::ConnectResponse(conn_resp) => {
								match conn_resp {
									ConnectionResponse::Error(addr, err) => log::error!("Error connecting to {}: {:?}", addr, err),
									ConnectionResponse::Established(conn) => {
										let node_idx = self.index_by_addr(&conn.addr)?;
										let session_info = self.gen_session_info();
										let handle = self.remote_mut(node_idx)?;
										handle.activate(node_action, conn, session_info).await;
									}
									ConnectionResponse::NotFound(addr) => log::warn!("No host found: {}", addr),
								}
							},
							NetEvent::Incoming(conn) => {
								let index = self.index_by_addr(&conn.addr)?;
								let session_info = self.gen_session_info();
								let handle = self.remote_mut(index)?;
								handle.activate(node_action, conn, session_info).await;
							}
							// Handle unprompted connection
							/* NetEvent::Incoming(addr, connection) => {
								self.handle_connection(&action_sender, addr, connection).await?;
							}, */
							// Handle user action
							NetEvent::UserAction(user_action) => {
								match user_action {
									UserAction::GetNodeInfo => {
										let node_info = net::NodeInfo {
											node_id: self.node_id.clone(),
											route_coord: self.route_coord.clone(),
											public_addr: self.public_addr.clone(),
											remotes: self.remotes.len(),
											active_remotes: self.direct_sorted.len(),
										};
										network_action.send(NetAction::UserEvent(UserEvent::NodeInfo(node_info))).await?;
									}
									_ => { log::error!("Received Unhandled UserAction: {:?}", user_action) }
								}
							}
							_ => { log::error!("Received Unhandled NetEvent: {:?}", net_event) }
						}
					}
					_ => { log::error!("Received Unused NodeAction<Net>: {:?}", action) },
				}
			};
			if node_error.is_err() {
				log::error!("Node Error: {:?}", node_error);
			}
		}

		self
	}

	// Handle Connection object by creating a new Remote object if it doesn't already exist and setting up mapping
	/* pub async fn handle_connection(&mut self, action_sender: &Sender<NodeAction<Net>>, address: Net::Address, connection: Net::Conn) -> Result<(), NodeError<Net>> {
		let remote = if self.addrs.contains_key(&address) {
			*self.addrs.get(&address).unwrap()
		} else {
			self.remotes.insert(RemoteHandle::Inactive(Remote::new()))
		};


		let remote = self.remote_mut(remote).unwrap();
		remote.activate(action_sender);
		Ok(())
	} */
}