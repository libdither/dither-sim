//! This is the Node Module, it defines all the behaviours of a Dither Node.
//! It provides a simple API to the internet module containing it.
#![allow(dead_code)]

#![feature(try_blocks)]
#![feature(arbitrary_self_types)]
#![feature(generic_associated_types)]
#![feature(associated_type_bounds)]

#[macro_use]
extern crate thiserror;

use async_std::{sync::Mutex, task::{self, JoinHandle}};
use futures::{SinkExt, StreamExt, channel::mpsc::{self, Receiver, Sender}};
use replace_with::replace_with_or_abort;

use std::{collections::{BTreeMap, HashMap}, fmt, sync::Arc, time::Instant};

use net::{Connection, NetAction, NetEvent, Network, UserAction, UserEvent};
pub use packet::NodePacket;

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
	Active { 
		shared_state: Arc<Mutex<Remote<Net>>>,
		join: JoinHandle<Remote<Net>>,
		sender: Sender<RemoteAction<Net>>,
	},
	Inactive { remote: Remote<Net> },
}

impl<Net: Network> RemoteHandle<Net> {
	pub fn new(remote: Remote<Net>) -> Self { Self::Inactive { remote } }
	/// Send RemoteAction to remote thread and create if thread doesn't exist.
	async fn spawn(&mut self, node_action: &Sender<NodeAction<Net>>, connection: Connection<Net>, session_info: SessionInfo) {
		// Safety: Self is initialized in the next line
		// let mut ret = mem::replace(self, unsafe { MaybeUninit::uninit().assume_init() });
		replace_with_or_abort(self, |mut ret| match ret {
			RemoteHandle::Inactive { remote } => {
				// Safety: Self is overwritten
				let (join, sender) = remote.clone().spawn(node_action.clone(), connection, session_info);
				RemoteHandle::Active { shared_state: Arc::new(Mutex::new(remote)), join, sender }
			},
			RemoteHandle::Active { .. } => {
				if let RemoteHandle::Active { sender, .. } = &mut ret {
					// Updates remote's active connection, or takes it out of bootstrapping mode
					sender.try_send(RemoteAction::HandleConnection(connection)).unwrap();
					sender.try_send(RemoteAction::UpdateInfo(session_info)).unwrap();
				}
				ret
			},
		});
	}
	async fn spawn_bootstrapping(&mut self, node_action: &Sender<NodeAction<Net>>, session_info: SessionInfo) {
		replace_with_or_abort(self, |ret| match ret {
			RemoteHandle::Inactive { remote } => {
				// Safety: Self is overwritten
				let (join, sender) = remote.clone().spawn_bootstrapping(node_action.clone(), session_info);
				RemoteHandle::Active { shared_state: Arc::new(Mutex::new(remote)), join, sender }
			},
			RemoteHandle::Active { .. } => { log::error!("Tried to boostrap to node, but Remote is already active"); ret }
		});
	}
	pub async fn action(&mut self, action: RemoteAction<Net>) -> Result<(), NodeError<Net>> {
		Ok(match self {
			RemoteHandle::Active { sender, .. } => {
				sender.send(action).await?
			}
			RemoteHandle::Inactive { .. } => Err(RemoteError::SessionInactive)?
		})
	}
	pub fn active(&self) -> bool { if let RemoteHandle::Active { .. } = self { true } else { false } }
	pub async fn connect(&mut self, connection: Connection<Net>) -> Result<(), NodeError<Net>> {
		self.action(RemoteAction::HandleConnection(connection)).await
	}
}
impl<Net: Network> fmt::Display for RemoteHandle<Net> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			RemoteHandle::Active { shared_state, .. } => {
				let remote = task::block_on(shared_state.lock());
				writeln!(f, "[*] {}", remote)
			}
			RemoteHandle::Inactive { remote } => {
				writeln!(f, "[ ] {}", remote)
			},
		}
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

	PrintNode,
}

#[derive(Error, Debug)]
pub enum NodeError<Net: Network> {
	// Error from Remote Node Thread
	#[error("Remote Error: {0}")]
	RemoteError(#[from] RemoteError),
	#[error("Connection error: {0}")]
	ConnectionError(Net::ConnectionError),
	
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

	/// Represents this node's listening address on the local network.
	pub local_addr: Option<Net::Address>,
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
			local_addr: None,
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
			let index = self.remotes.insert(RemoteHandle::new(Remote::new_direct(node_id.clone(), addr.clone())));
			self.addrs.insert(addr.clone(), index);
			self.ids.insert(node_id, index);
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
				log::debug!("Received node action: {:?}", action);
				match action {
					// Initiate Bootstrapping process
					NodeAction::Bootstrap(node_id, addr) => {
						log::debug!("Bootstrapping: {}, {}", node_id, addr);
						let session_info = self.gen_session_info();
						let handle = self.get_or_new_remote(node_id.clone(), &addr)?;
						handle.spawn_bootstrapping(node_action, session_info).await; // Spawn listening for an initial connection 
						network_action.send(NetAction::Connect(node_id, addr)).await?; // Attempt to connect
					}
					// Forward Net actions sent by remote
					NodeAction::NetAction(net_action) => network_action.send(net_action).await?,
					// Deal with any Network Events
					NodeAction::NetEvent(net_event) => {
						match net_event {
							// Handle requested connection
							NetEvent::ConnectResponse(conn_res) => {
								let conn = conn_res.map_err(|e|NodeError::ConnectionError(e))?;
								let node_idx = self.index_by_node_id(&conn.node_id)?;
								let handle = self.remote_mut(node_idx)?;
								handle.connect(conn).await?; // Update connection for existing node
							},
							// Handle unrequested connection
							NetEvent::Incoming(conn) => {
								let session_info = self.gen_session_info();
								let handle = self.get_or_new_remote(conn.node_id.clone(), &conn.addr)?;
								handle.spawn(node_action, conn, session_info).await; // Spawn node with connection
							}
							// Handle user action
							NetEvent::UserAction(user_action) => {
								match user_action {
									UserAction::GetNodeInfo => {
										let node_info = net::NodeInfo {
											node_id: self.node_id.clone(),
											route_coord: self.route_coord.clone(),
											local_addr: self.local_addr.clone(),
											public_addr: self.public_addr.clone(),
											remotes: self.remotes.len(),
											active_remotes: self.direct_sorted.len(),
										};
										network_action.send(NetAction::UserEvent(UserEvent::NodeInfo(node_info))).await?;
									}
									_ => { log::error!("Received Unhandled UserAction: {:?}", user_action) }
								}
							}
							// _ => log::error!("Received Unhandled NetEvent: {:?}", net_event)
						}
					},
					NodeAction::PrintNode => {
						println!("{}", self);
					},
					NodeAction::ForwardPacket(node_id, packet) => {
						let handle = self.remote_mut(self.index_by_node_id(&node_id)?)?;
						handle.action(RemoteAction::SendPacket(packet)).await?;
					}
					_ => { log::error!("Received Unused NodeAction<Net>: {:?}", action) },
				}
			};
			if let Err(err) = node_error {
				log::error!("Node Error: {}", err);
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

impl<Net: Network> fmt::Display for Node<Net> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		writeln!(f, "\nNode({})", self.node_id)?;
		writeln!(f, "	local_addr: {:?}", self.local_addr)?;
		writeln!(f, "	public_addr: {:?}", self.public_addr)?;
		writeln!(f, "	route_coord: {:?}", self.route_coord)?;
		writeln!(f, "	total_nodes: {:?}", self.remotes.len())?;
		// writeln!(f, "start_time: {}", self.start_time)?;
		for (idx, remote) in &self.remotes {
			write!(f, "	{:?} {}", idx, remote)?;
		}

		Ok(())
	}
}