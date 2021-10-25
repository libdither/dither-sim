//! This is the Node Module, it defines all the behaviours of a Dither Node.
//! It provides a simple API to the internet module containing it.
#![allow(dead_code)]

#![feature(drain_filter)]
#![feature(backtrace)]
#![feature(try_blocks)]
#![feature(arbitrary_self_types)]
#![feature(unzip_option)]
#![feature(destructuring_assignment)]

#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate rkyv;
#[macro_use]
extern crate derivative;
use serde;

use std::{collections::{BTreeMap, HashMap}, ops::{DerefMut}, time::Duration};
use session::Session;
use tokio::{sync::mpsc::{self, Receiver, Sender}, task::JoinHandle};

use net::{Address, Connection, ConnectionResponse, NetAction};
use packet::NodePacket;

pub mod net; // Fundamental network types;

mod packet;
mod remote;
mod session;
mod types;
// mod rkyv_codec;

use remote::{RemoteNode, RemoteAction, RemoteNodeError};
pub use types::{NodeID, RouteCoord, RouteScalar};

use slotmap::{SlotMap, new_key_type};

new_key_type! { pub struct RemoteIdx; }

/// Structure that holds information relevant only to this Node about Remote Nodes.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Remote {
	pub node_id: Option<NodeID>,

	pub address: net::Address,

	pub route_coord: Option<RouteCoord>,

	pub session: Option<Session>,

	#[serde(skip)]
	pub action_sender: Option<(Sender<RemoteAction>, JoinHandle<Result<(), RemoteNodeError>>)>,
}

impl Remote {
	pub fn new(address: net::Address, node_id: Option<NodeID>) -> Self {
		Self {
			node_id,
			address,
			route_coord: None,
			session: None,
			action_sender: None,
		}
	}
	/// Send RemoteAction to remote thread and create if thread doesn't exist.
	pub async fn action(&mut self, node_action: &Sender<NodeAction>, action: RemoteAction) -> Result<(), mpsc::error::SendError<RemoteAction>> {
		let action_sender: &Sender<RemoteAction> = if let Some(action_sender) = &self.action_sender {
			&action_sender.0
		} else {
			let (action_sender, action_receiver) = mpsc::channel(20);
			let self_immutable = &*self;
			let remote_node = RemoteNode::new(Some(self_immutable), action_sender.clone());

			let node_action = node_action.clone();
			let join_handle = tokio::spawn(async move {
				remote_node.run(action_receiver, node_action).await
			});

			&self.action_sender.insert((action_sender, join_handle)).0
		};
		action_sender.send(action).await
	}
}


/// Actions that can be run by an external entity (either the internet implementation or the user)
#[derive(Debug)]
pub enum NodeAction {
	/// Bootstrap this node onto a specific other network node, starts the self-organization process
	Bootstrap(NodeID, net::Address),

	/// Connect to network through passed sim::Connection
	/// Initiate Handshake with remote NodeID, net::Address and initial packets
	//Connect(net::Connection, NodeID, SessionType, Vec<NodePacket>),

	/// Handle Incoming action (from Internet)
	//#[serde(skip)]
	NetAction(net::NetAction),

	UpdateRemote(NodeID, Option<RouteCoord>, usize, u64),
	/// Request Peers of another node to ping me
	RequestPeers(NodeID, usize),
	/// Try and calculate route coordinate using Principle Coordinate Analysis of closest nodes (MDS)
	CalcRouteCoord,
	/// Exchange Info with another node
	ExchangeInformation(NodeID),
	/// Organize and set/unset known nodes as peers for Routing
	CalculatePeers,
	/// Sends a packet out onto the network for a specific recipient
	Notify(NodeID, u64),
	/// Send DHT request for Route Coordinate
	RequestRouteCoord(NodeID),
	/// Establish Traversed Session with remote NodeID
	/// Looks up remote node's RouteCoord on DHT and enables Traversed Session
	ConnectTraversed(NodeID, Vec<NodePacket>),
	/// Establishes Routed session with remote NodeID
	/// Looks up remote node's RouteCoord on DHT and runs CalculateRoute after RouteCoord is received
	/// * `usize`: Number of intermediate nodes to route through
	/// * `f64`: Random intermediate offset (high offset is more anonymous but less efficient, very high offset is random routing strategy)
	ConnectRouted(NodeID, usize),
	/// Send specific packet to node
	SendData(NodeID, Vec<u8>),
}

#[derive(Error, Debug)]
pub enum NodeError {
	// Error from Remote Node Thread
	#[error(transparent)]
	RemoteNodeError(#[from] RemoteNodeError),
	#[error("RemoteNode mpsc channel backed up!")]
	SendRemoteActionError(#[from] mpsc::error::SendError<RemoteAction>),

	// When Accessing Remotes
	#[error("Unknown Node Index: {node_idx:?}")]
	UnknownNodeIndex { node_idx: RemoteIdx },
	#[error("Unknown NodeID: {node_id:?}")]
	UnknownNodeID { node_id: NodeID },
	#[error("Unknown Network Addr: {net_addr:?}")]
	UnknownNetAddr { net_addr: Address },

	#[error("There is no calculated route coordinate for this node")]
	NoCalculatedRouteCoord,
	#[error("There are not enough peers, needed: {required}")]
	InsufficientPeers { required: usize },

	// Catch-all
	#[error(transparent)]
	Other(#[from] anyhow::Error),

}
impl NodeError {
	pub fn anyhow(self) -> NodeError {
		NodeError::Other(anyhow::Error::new(self))
	}
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Node {
	/// Universally Unique Identifier of a Node. In the future this will be the Multihash of the public key
	pub node_id: NodeID,

	/// Represents what this node is identified as on the network implementation. In real life, there would be multiple of these but for testing purposes there will just be one.
	pub net_addr: Option<net::Address>,

	/// This node's Distance-Based Routing Coordinates
	pub route_coord: Option<RouteCoord>,

	/// Whether this node's routing coordinate is published directly to the DHT. (not needed for testing, might change)
	is_public: bool,

	/// This node's public routing coordinate(s) published to the DHT (there will be support for multiple in later versions).
	#[derivative(Debug = "ignore")]
	#[allow(dead_code)]
	public_route: Option<RouteCoord>,

	/// Amount of time passed since startup of this node
	pub ticks: Duration,

	/// Hold Info about remote nodes
	remotes: SlotMap<RemoteIdx, Remote>,
	/// Map NodeIDs to Remote Node Idicies
	ids: HashMap<NodeID, RemoteIdx>,

	/// Map Addresses to Remote Node Idicies
	//#[serde(skip)]
	addrs: HashMap<Address, RemoteIdx>,

	/// Sorted list of nodes based on how close they are latency-wise
	direct_sorted: BTreeMap<u64, RemoteIdx>, // All nodes that have been tested, sorted by lowest value

	//pub peer_list: BiHashMap<RemoteIdx, RouteCoord>, // Used for routing and peer management, peer count should be no more than TARGET_PEER_COUNT
	
	/* /// Bi-directional graph of all locally known nodes and the estimated distances between them
	#[derivative(Debug = "ignore")]
	//#[serde(skip)]
	route_map: DiGraphMap<NodeID, u64>,  */


	/// Send Actions to the Network
	#[derivative(Debug = "ignore")]
	//#[serde(skip)]
	network_action: Sender<NetAction>,

	/// Event Loop Receive & Send NodeActions
	#[derivative(Debug = "ignore")]
	//#[serde(skip)]
	action_receiver: Receiver<NodeAction>,
	#[derivative(Debug = "ignore")]
	//#[serde(skip)]
	pub action_sender: Sender<NodeAction>,
}

impl Node {
	/// Create New Node with specific ID
	pub fn new(node_id: NodeID, network_event_sender: Sender<NetAction>) -> Node {
		let (action_sender, action_receiver) = mpsc::channel(20);
		Node {
			node_id,
			net_addr: None,
			route_coord: None,
			is_public: true,
			public_route: None,
			ticks: Duration::ZERO,
			remotes: Default::default(),
			ids: Default::default(),
			addrs: Default::default(),
			direct_sorted: Default::default(),
			//route_map: Default::default(),
			network_action: network_event_sender,
			action_receiver,
			action_sender,
		}
	}

	/// Add action to Node object
	pub fn with_action(self, action: NodeAction) -> Self {
		self.action_sender.try_send(action).unwrap();
		self
	}

	pub fn remote(&self, node_idx: RemoteIdx) -> Result<&Remote, NodeError> {
		self.remotes
			.get(node_idx)
			.ok_or(NodeError::UnknownNodeIndex { node_idx })
	}
	pub fn remote_mut(&mut self, node_idx: RemoteIdx) -> Result<&mut Remote, NodeError> {
		self.remotes
			.get_mut(node_idx)
			.ok_or(NodeError::UnknownNodeIndex { node_idx })
	}
	pub fn index_by_node_id(&self, node_id: &NodeID) -> Result<RemoteIdx, NodeError> {
		self.ids
			.get(node_id)
			.cloned()
			.ok_or(NodeError::UnknownNodeID {
				node_id: node_id.clone(),
			})
	}
	pub fn index_by_addr(&self, net_addr: &Address) -> Result<RemoteIdx, NodeError> {
		self.addrs
    		.get(net_addr)
			.cloned()
			.ok_or(NodeError::UnknownNetAddr {
				net_addr: net_addr.clone(),
			})
	}

	/// Runs event loop on this object
	pub async fn run<S: DerefMut<Target=Node>>(mut self: S) {
		let node_action = &self.action_sender.clone();

		//let node = self.deref_mut();
		while let Some(action) = self.action_receiver.recv().await {
			let node_error: Result<(), NodeError> = try {
				match action {
					NodeAction::NetAction(net_action) => {
						match net_action {
							NetAction::Incoming(connection) => {
								self.handle_connection(connection).await?;
							},
							NetAction::QueryRouteCoordResponse(node_id, route_coord) => {
								let node_idx = self.index_by_node_id(&node_id)?;
								self.remote_mut(node_idx)?.action(node_action, RemoteAction::RouteCoordQuery(route_coord)).await?;
							}
							NetAction::ConnectResponse(conn_resp) => {
								if let ConnectionResponse::Established(connection) = conn_resp {
									self.handle_connection(connection).await?;
								}
							},
							_ => { log::error!("Received Invalid NetAction: {:?}", net_action) }
						}
					}
					_ => { log::error!("Received Unused NodeAction: {:?}", action) },
				}
			};
			if node_error.is_err() {
				log::error!("Node Error: {:?}", node_error);
			}
		}
	}

	/// Handle Connection object by creating a new Remote object if it doesn't already exist 
	pub async fn handle_connection(&mut self, connection: Connection) -> Result<(), NodeError> {
		let remote = if self.addrs.contains_key(&connection.address) {
			*self.addrs.get(&connection.address).unwrap()
		} else {
			self.remotes.insert(Remote::new(connection.address.clone(), None))
		};

		let node_action = &self.action_sender.clone();
		let remote = self.remote_mut(remote).unwrap();
		remote.action(node_action, RemoteAction::HandleConnection(connection)).await?;
		Ok(())
	}

	pub fn spawn(mut self) -> JoinHandle<Self> {
		tokio::spawn(async move {
			Self::run(&mut self).await;
			self
		})
	}
}