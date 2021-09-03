//! This is the Node Module, it defines all the behaviours of a Dither Node.
//! It provides a simple API to the internet module containing it.
#![allow(unused_imports)]
#![feature(drain_filter)]
#![feature(backtrace)]
#![feature(try_blocks)]
#![feature(arbitrary_self_types)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate rkyv;
#[macro_use]
extern crate derivative;

const TARGET_PEER_COUNT: usize = 10;

use std::{collections::{BTreeMap, HashMap}, ops::{Deref, DerefMut}, time::Duration};
use async_std::{channel::{self, Receiver, Sender}, task};
use nalgebra::{Point, Vector2};
use net::{Connection, NetAction};
use packet::NodePacket;

pub mod net; // Fundamental network types;

mod packet;
mod remote;
mod session;
mod types;

use remote::{RemoteNode, RemoteAction, RemoteNodeError};
pub use types::{NodeID, RouteCoord, RouteScalar};

use bimap::BiHashMap;
use petgraph::graphmap::DiGraphMap;
use slotmap::{SlotMap, new_key_type};
use smallvec::SmallVec;

new_key_type! { pub struct RemoteIdx; }

/// Structure that holds information relevant only to this Node about Remote Nodes.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Remote {
	pub node_id: Option<NodeID>,

	pub address: net::Address, 

	#[serde(skip)]
	pub action_sender: Sender<RemoteAction>,
}

impl Remote {
	pub async fn action(&mut self, action: RemoteAction) {
		self.action_sender.send(action).await;
	}
}

#[derive(Debug, Clone)]
pub enum NodeAction {
	/// Actions that can be run by an external entity (either the internet implementation or the user)

	/// Bootstrap this node onto a specific other network node, starts the self-organization process
	Bootstrap(NodeID, net::Address),

	/// Connect to network through passed sim::Connection
	/// Initiate Handshake with remote NodeID, net::Address and initial packets
	//Connect(net::Connection, NodeID, SessionType, Vec<NodePacket>),

	/// Handle Incoming action (from Internet)
	HandleNetAction(net::NetAction),

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

	// When Accessing Remotes
	#[error("Unknown Node Index: {node_idx:?}")]
	UnknownNodeIndex { node_idx: RemoteIdx },
	#[error("Unknown NodeID: {node_id:?}")]
	UnknownNodeID { node_id: NodeID },

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

#[derive(Derivative, serde::Serialize, serde::Deserialize)]
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
	public_route: Option<RouteCoord>,

	/// Amount of time passed since startup of this node
	pub ticks: Duration,

	/// Hold Info about remote nodes
	remotes: SlotMap<RemoteIdx, Remote>,
	/// Map NodeIDs to Remote Node Idicies
	ids: HashMap<NodeID, RemoteIdx>,

	/// Sorted list of nodes based on how close they are latency-wise
	direct_sorted: BTreeMap<u64, RemoteIdx>, // All nodes that have been tested, sorted by lowest value

	//pub peer_list: BiHashMap<RemoteIdx, RouteCoord>, // Used for routing and peer management, peer count should be no more than TARGET_PEER_COUNT
	
	/// Bi-directional graph of all locally known nodes and the estimated distances between them
	#[derivative(Debug = "ignore")]
	#[serde(skip)]
	route_map: DiGraphMap<NodeID, u64>, 

	/// Send Actions to the Network
	#[derivative(Debug = "ignore")]
	#[serde(skip)]
	network_action: Sender<NetAction>,

	/// Event Loop Receive & Send NodeActions
	#[derivative(Debug = "ignore")]
	#[serde(skip)]
	action_receiver: Receiver<NodeAction>,
	pub action_sender: Sender<NodeAction>,
}

impl Node {
	/// Create New Node with specific ID
	pub fn new(node_id: NodeID, network_event_sender: Sender<NetAction>) -> Node {
		let (action_sender, action_receiver) = channel::bounded(20);
		Node {
			node_id,
			net_addr: None,
			route_coord: None,
			is_public: true,
			public_route: None,
			ticks: Duration::ZERO,
			remotes: Default::default(),
			ids: Default::default(),
			direct_sorted: Default::default(),
			route_map: Default::default(),
			network_action: network_event_sender,
			action_receiver,
			action_sender,
		}
	}

	/// Add action to Node object
	pub fn with_action(mut self, action: NodeAction) -> Self {
		self.action_list.push(action);
		self
	}

	pub fn remote(&self, node_idx: RemoteIdx) -> Result<&Remote, NodeError> {
		self.remotes
			.get(node_idx)
			.ok_or(NodeError::InvalidNodeIndex { node_idx })
	}
	pub fn remote_mut(&mut self, node_idx: RemoteIdx) -> Result<&mut Remote, NodeError> {
		self.remotes
			.get_mut(node_idx)
			.ok_or(NodeError::InvalidNodeIndex { node_idx })
	}
	pub fn index_by_node_id(&self, node_id: &NodeID) -> Result<RemoteIdx, NodeError> {
		self.ids
			.get_by_left(node_id)
			.cloned()
			.ok_or(NodeError::InvalidNodeID {
				node_id: node_id.clone(),
			})
	}

	pub fn find_closest_peer(&self, remote_route_coord: &RouteCoord) -> Result<RemoteIdx, NodeError> {
		let min_peer = self.peer_list.iter().min_by_key(|(_, &p)| {
			let diff = p - *remote_route_coord;
			diff.dot(&diff)
			//println!("Dist from {:?}: {}: {}", self.node_id, self.remote(**id).unwrap().node_id, d_sq);
			//d_sq
		});
		min_peer
			.map(|(&node, _)| node)
			.ok_or(NodeError::InsufficientPeers { required: 1 })
	}

	/// Runs event loop on this object
	pub async fn run<S: DerefMut<Target=Node>>(self: S) -> Self {
		//let node = self.deref_mut();
		while let Ok(action) = self.action_receiver.recv().await {
			let node_error = try {
				match action {
					NodeAction::Bootstrap(node_id, net_addr) => {
	
					},
					//NodeAction::Connect(connection) => self.handle_connection(connection),
					NodeAction::HandleNetAction(net_action) => {
						match net_action {
							NetAction::Incoming(connection) => self.handle_connection(connection),
							NetAction::QueryRouteCoordResponse(node_id, route_coord) => {
								let node_idx = self.index_by_node_id(&node_id)?;
								self.remote(node_idx)?.action.send(RemoteAction::QueryRouteCoordResponse(route_coord)).await;
							}
							NetAction::ConnectResponse(connection) => self.handle_connection(connection),
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
	/// Handle Connection object from Network Implementation by creating Remote Node Thread
	pub fn handle_connection(&mut self, connection: Connection) {
		// Create Remote
		let (remote_node, remote) = RemoteNode::new(connection);

		// Register Remote
		let remote_node_id = remote.node_id;
		let node_idx = self.remotes.insert(remote);
		self.ids.insert(remote_node_id, node_idx);

		// Spawn Remote Task
		task::spawn(async {
			remote_node.run(self.action_sender).await;
		});
	}
	/// Initiate handshake process and send packets when completed
	pub fn connect(&mut self, node_id: NodeID, net_addr: net::Address) -> Result<(), NodeError> {
		self.network_action.send(NetAction::Connect(net_addr))
		//let (remote_node, remote) = RemoteNode::new_outgoing(node_id, net_addr);

	}
}