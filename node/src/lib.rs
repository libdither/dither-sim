//! This is the Node Module, it defines all the behaviours of a Dither Node.
//! It provides a simple API to the internet module containing it.

#![allow(unused_imports)]
#![feature(drain_filter)]
#![feature(backtrace)]
#![feature(try_blocks)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate serde;

const TARGET_PEER_COUNT: usize = 10;

use std::{collections::BTreeMap, ops::{Deref, DerefMut}, time::Duration};
use async_std::{channel::{self, Receiver, Sender}, task};
use nalgebra::{Point, Vector2};
use serde::{Serialize, Deserialize};

pub mod net; // Fundamental network types;

mod packet;
mod remote;
mod session;
mod types;

use remote::{RemoteNode, RemoteAction, RemoteNodeError};
pub use types::{NodeID, RouteCoord, RouteScalar};

use bimap::BiHashMap;
use petgraph::graphmap::DiGraphMap;
use slotmap::SlotMap;
use smallvec::SmallVec;

new_key_type! { pub struct RemoteIdx; }

/// Structure that holds information relevant only to this Node about Remote Nodes.
#[derive(Debug, Serialize, Deserialize)]
pub struct Remote {
	pub node_id: Option<NodeID>,

	pub address: net::Address, 

	#[serde(skip)]
	pub action_sender: Sender<RemoteAction>,
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

#[derive(Derivative, Serialize, Deserialize)]
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
				log::error!("Node Error: {:?}");
			}
		}
	}

	/// Handle Connection object from Network Implementation by creating Remote Node Thread
	fn handle_connection(&mut self, connection: Connection) {
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

	// Returns true if action should be deleted and false if it should not be
	pub fn parse_action(
		&mut self,
		action: NodeAction,
		outgoing: &mut PacketVec,
		out_actions: &mut ActionVec,
	) -> Result<Option<NodeAction>, NodeError> {
		log::trace!(
			"[{: >6}] NodeID({}) Running Action: {:?}",
			self.ticks,
			self.node_id,
			action
		);
		match action {
			NodeAction::Bootstrap(remote_node_id, net_addr) => {
				self.connect(
					remote_node_id,
					SessionType::direct(net_addr),
					vec![NodePacket::ExchangeInfo(self.route_coord, 0, 0)],
					outgoing,
				)?;
			}
			NodeAction::Connect(remote_node_id, session_type, ref packets) => {
				self.connect(remote_node_id, session_type, packets.clone(), outgoing)?;
			}
			NodeAction::UpdateRemote(
				remote_node_id,
				remote_route_coord,
				remote_direct_count,
				remote_ping,
			) => {
				self.route_map
					.add_edge(remote_node_id, self.node_id, remote_ping);

				let self_route_coord = self.route_coord;

				// Record Remote Coordinate
				let node_idx = self.index_by_node_id(&remote_node_id)?;
				let remote = self.remote_mut(node_idx)?;
				let mut did_route_change = remote.route_coord != remote_route_coord;
				remote.route_coord = remote_route_coord;

				// If this node has coord,
				if let None = self.route_coord {
					out_actions.push(NodeAction::CalcRouteCoord);
					did_route_change = false;
				}
				if did_route_change {
					out_actions.push(NodeAction::CalculatePeers);
				}
				// If need more peers & remote has a peer, request pings
				if self.direct_sorted.len() < TARGET_PEER_COUNT && remote_direct_count >= 2 {
					self.send_packet(
						node_idx,
						NodePacket::RequestPings(TARGET_PEER_COUNT, self_route_coord),
						outgoing,
					)?;
				}
			}
			NodeAction::CalcRouteCoord => {
				self.route_coord = Some(self.calculate_route_coord()?);
				out_actions.push(NodeAction::CalculatePeers);
			}
			NodeAction::ExchangeInformation(remote_node_id) => {
				let node_idx = self.index_by_node_id(&remote_node_id)?;
				let avg_dist = self.remote(node_idx)?.session()?.tracker.dist_avg;
				self.send_packet(
					node_idx,
					NodePacket::ExchangeInfo(self.route_coord, self.peer_list.len(), avg_dist),
					outgoing,
				)?;
			}
			NodeAction::CalculatePeers => {
				// Collect the viable peers
				let self_route_coord = self.route_coord.ok_or(NodeError::NoCalculatedRouteCoord)?;
				let direct_nodes = self
					.direct_sorted
					.iter()
					.map(|s| s.1.clone())
					.collect::<Vec<RemoteIdx>>();
				self.peer_list = direct_nodes
					.iter()
					.filter_map(|&node_idx| {
						// Decides whether remote should be added to peer list
						self.remote(node_idx)
							.ok()
							.map(|remote| {
								if let Some(route_coord) = remote.is_viable_peer(self_route_coord) {
									Some((node_idx, route_coord))
								} else {
									None
								}
							})
							.flatten()
					})
					.take(TARGET_PEER_COUNT)
					.collect();

				// Notify Peers if just became peer
				let num_peers = self.peer_list.len();
				for node_idx in direct_nodes {
					let toggle = self.peer_list.contains_left(&node_idx);
					let remote = self.remote(node_idx)?;
					let dist = remote.session()?.tracker.dist_avg;
					match (!remote.session()?.is_peer(), toggle) {
						(false, true) => {
							// Notify that this node thinks of other node as a direct peer
							self.send_packet(
								node_idx,
								NodePacket::PeerNotify(0, self_route_coord, num_peers, dist),
								outgoing,
							)?;
						}
						(true, false) => {
							// Notify that this node no longer things of other node as a direct peer, so perhaps other node should drop connection
							self.send_packet(
								node_idx,
								NodePacket::PeerNotify(
									usize::MAX,
									self_route_coord,
									num_peers,
									dist,
								),
								outgoing,
							)?;
						}
						_ => {}
					}
					self.remote_mut(node_idx)?
						.session_mut()?
						.direct_mut()?
						.set_peer(toggle);
				}

				// If have enough peers & want to host node as public, write RouteCoord to DHT
				if self.peer_list.len() >= TARGET_PEER_COUNT
					&& self.is_public && self.public_route != self.route_coord
				{
					self.public_route = self.route_coord;
					outgoing.push(InternetPacket::gen_request(
						self.net_addr,
						InternetRequest::RouteCoordDHTWrite(self.node_id, self_route_coord),
					));
				}
			}
			NodeAction::Notify(remote_node_id, data) => {
				let remote = self.remote(self.index_by_node_id(&remote_node_id)?)?;
				if remote.route_coord.is_some() {
					let encryption = NodeEncryption::Notify {
						recipient: remote_node_id,
						data,
						sender: self.node_id,
					};
					outgoing.push(remote.session()?.gen_packet(encryption, self)?)
				} else {
					out_actions.push(NodeAction::RequestRouteCoord(remote_node_id));
					out_actions.push(
						NodeAction::Notify(remote_node_id, data)
							.gen_condition(NodeActionCondition::RemoteRouteCoord(remote_node_id)),
					);
				}
			}
			NodeAction::RequestRouteCoord(remote_node_id) => {
				outgoing.push(InternetPacket::gen_request(
					self.net_addr,
					InternetRequest::RouteCoordDHTRead(remote_node_id),
				));
			}
			NodeAction::ConnectTraversed(remote_node_id, packets) => {
				let (_, remote) = self.add_remote(remote_node_id)?;
				if let Some(remote_route_coord) = remote.route_coord {
					self.connect(
						remote_node_id,
						SessionType::traversed(remote_route_coord),
						packets,
						outgoing,
					)?;
				} else {
					// Wait for RouteCoord DHT to resolve before re-running
					out_actions.push(NodeAction::RequestRouteCoord(remote_node_id));
					out_actions.push(
						NodeAction::ConnectTraversed(remote_node_id, packets)
							.gen_condition(NodeActionCondition::RemoteRouteCoord(remote_node_id)),
					);
				}
			}
			NodeAction::ConnectRouted(remote_node_id, hops) => {
				let self_route_coord = self.route_coord.ok_or(NodeError::NoCalculatedRouteCoord)?;
				// Check if Remote Route Coord was allready requested
				let (_, remote) = self.add_remote(remote_node_id.clone())?;
				if let Some(remote_route_coord) = remote.route_coord {
					let self_route_coord = self_route_coord.map(|s| s as f64);
					let remote_route_coord = remote_route_coord.map(|s| s as f64);
					let diff = (remote_route_coord - self_route_coord) / hops as f64;
					let mut routes = Vec::with_capacity(hops);
					for i in 1..hops {
						routes.push(self_route_coord + diff * i as f64);
					}
					println!("Routes: {:?}", routes);
				//use nalgebra::distance_squared;
				// Find nearest node
				//let nearest_peer = self.peer_list.iter().min_by_key(|(id,&r)|distance_squared(&routes[0], &r.map(|s|s as f64)) as i64);

				//self.routed_connect(remote_node_id, outgoing);
				//self.remote_mut(self.index_by_node_id(&remote_node_id)?)?.connect_routed(routes);
				} else {
					// Otherwise, Request it and await Condition for next ConnectRouted
					out_actions.push(NodeAction::RequestRouteCoord(remote_node_id));
					out_actions.push(
						NodeAction::ConnectRouted(remote_node_id, hops)
							.gen_condition(NodeActionCondition::RemoteRouteCoord(remote_node_id)),
					);
				}
			}
			NodeAction::SendData(remote_node_id, data) => {
				self.send_packet(
					self.index_by_node_id(&remote_node_id)?,
					NodePacket::Data(data),
					outgoing,
				)?;
			}
			NodeAction::Condition(condition, embedded_action) => {
				// Returns embedded action if condition is satisfied (e.g. check() returns true), else returns false to prevent action from being deleted
				if condition.check(self)? {
					return Ok(Some(*embedded_action));
				} else {
					return Ok(Some(NodeAction::Condition(condition, embedded_action)));
				}
			}
			_ => {
				unimplemented!("Unimplemented Action")
			}
		}
		//log::trace!("[{: >6}] NodeID({}) Completed Action: {:?}", self.ticks, self.node_id, action);
		Ok(None) // By default don't return action
	}
	pub fn parse_node_packet(
		&mut self,
		return_node_idx: RemoteIdx,
		received_packet: NodePacket,
		outgoing: &mut PacketVec,
	) -> Result<(), NodeError> {
		let self_ticks = self.ticks;
		let return_remote = self.remote_mut(return_node_idx)?;
		let return_node_id = return_remote.node_id;
		let packet_last_received = return_remote.session_mut()?.check_packet_time(
			&received_packet,
			return_node_id,
			self_ticks,
		);

		log::debug!(
			"[{: >6}] Node({}) received NodePacket::{:?} from NodeID({})",
			self.ticks,
			self.node_id,
			received_packet,
			return_node_id
		);

		match received_packet {
			NodePacket::ConnectionInit(ping_id, packets) => {
				// Acknowledge ping
				let distance = self
					.remote_mut(return_node_idx)?
					.session_mut()?
					.tracker
					.acknowledge_ping(ping_id, self_ticks)?;
				self.route_map
					.add_edge(self.node_id, return_node_id, distance);
				self.direct_sorted.insert(distance, return_node_idx);
				// Recursively parse packets
				for packet in packets {
					self.parse_node_packet(return_node_idx, packet, outgoing)?;
				}
			}
			NodePacket::ExchangeInfo(remote_route_coord, _remote_direct_count, remote_ping) => {
				if self.node_id == 0 && self.direct_sorted.len() == 1 && self.route_coord.is_none()
				{
					self.route_coord = Some(self.calculate_route_coord()?);
				}

				// Note Data, Update Remote
				self.action(NodeAction::UpdateRemote(
					return_node_id,
					remote_route_coord,
					_remote_direct_count,
					remote_ping,
				));

				// Send Return Packet
				let route_coord = self.route_coord;
				let peer_count = self.direct_sorted.len();
				let remote = self.remote_mut(return_node_idx)?;
				let ping = remote.session()?.tracker.dist_avg;
				self.send_packet(
					return_node_idx,
					NodePacket::ExchangeInfoResponse(route_coord, peer_count, ping),
					outgoing,
				)?;
			}
			NodePacket::ExchangeInfoResponse(
				remote_route_coord,
				remote_direct_count,
				remote_ping,
			) => {
				self.action(NodeAction::UpdateRemote(
					return_node_id,
					remote_route_coord,
					remote_direct_count,
					remote_ping,
				));
			}
			NodePacket::ProposeRouteCoords(route_coord_proposal, remote_route_coord_proposal) => {
				let acceptable = if self.route_coord.is_none() {
					self.route_coord = Some(route_coord_proposal);
					self.remote_mut(return_node_idx)?.route_coord =
						Some(remote_route_coord_proposal);
					true
				} else {
					false
				};
				self.send_packet(
					return_node_idx,
					NodePacket::ProposeRouteCoordsResponse(
						route_coord_proposal,
						remote_route_coord_proposal,
						acceptable,
					),
					outgoing,
				)?;
			}
			NodePacket::ProposeRouteCoordsResponse(
				initial_remote_proposal,
				initial_self_proposal,
				accepted,
			) => {
				if accepted {
					self.route_coord = Some(initial_self_proposal);
					self.remote_mut(return_node_idx)?.route_coord = Some(initial_remote_proposal);
				}
			}
			NodePacket::RequestPings(requests, requester_route_coord) => {
				if let Some(time) = packet_last_received {
					if time < 2000 {
						return Ok(());
					}
				} // Nodes should not be spamming this multiple times
				// Loop through first min(N,MAX_REQUEST_PINGS) items of priorityqueue
				let num_requests = usize::min(requests, MAX_REQUEST_PINGS); // Maximum of 10 requests

				// TODO: Use vpsearch Tree datastructure for optimal efficiency
				// Locate closest nodes (TODO: Locate nodes that have a wide diversity of angles for optimum efficiency)
				self.remote_mut(return_node_idx)?.route_coord = requester_route_coord;
				let closest_nodes = if let Some(route_coord) = requester_route_coord {
					let point_target = route_coord.map(|s| s as f64);
					let mut sorted = self
						.direct_sorted
						.iter()
						.filter_map(|(&_, &node_idx)| {
							self.remote(node_idx).ok().map(|remote| {
								if let Some(p) = remote.route_coord {
									Some((node_idx, nalgebra::distance_squared(&p.map(|s| s as f64), &point_target) as u64))
								} else { None }
							}).flatten()
						})
						.collect::<Vec<(RemoteIdx, u64)>>();
					sorted.sort_unstable_by_key(|k| k.1);
					sorted
						.iter()
						.map(|(node, _)| node.clone())
						.take(num_requests)
						.collect()
				} else {
					self.direct_sorted
						.iter()
						.map(|(_, node)| node.clone())
						.take(num_requests)
						.collect::<Vec<RemoteIdx>>()
				};

				// Send WantPing packet to first num_requests of those peers
				let want_ping_packet = NodePacket::WantPing(
					return_node_id,
					self.remote(return_node_idx)?.session()?.direct()?.net_addr,
				);
				for node_idx in closest_nodes {
					//let remote = self.remote(&node_id)?;
					if self.remote(node_idx)?.node_id != return_node_id {
						self.send_packet(node_idx, want_ping_packet.clone(), outgoing)?;
					}
				}
			}
			NodePacket::WantPing(requesting_node_id, requesting_net_addr) => {
				// Only send WantPing if this node is usedful
				if self.node_id == requesting_node_id || self.route_coord.is_none() {
					return Ok(());
				}
				let distance_self_to_return =
					self.remote(return_node_idx)?.session()?.tracker.dist_avg;

				let (_, request_remote) = self.add_remote(requesting_node_id)?;
				if let Ok(_request_session) = request_remote.session() {
					// If session, ignore probably
					return Ok(());
				} else {
					// If no session, send request
					if request_remote.pending_session.is_none() {
						self.action(NodeAction::Connect(
							requesting_node_id,
							SessionType::direct(requesting_net_addr),
							vec![NodePacket::AcceptWantPing(
								return_node_id,
								distance_self_to_return,
							)],
						));
					}
				}
			}
			NodePacket::AcceptWantPing(intermediate_node_id, return_to_intermediate_distance) => {
				let avg_dist = self.remote(return_node_idx)?.session()?.dist();
				self.route_map.add_edge(
					return_node_id,
					intermediate_node_id,
					return_to_intermediate_distance,
				);
				if let Some(time) = packet_last_received {
					if time < 300 {
						return Ok(());
					}
				}

				let self_route_coord = self.route_coord;
				let self_node_count = self.direct_sorted.len();
				self.send_packet(
					return_node_idx,
					NodePacket::ExchangeInfo(self_route_coord, self_node_count, avg_dist),
					outgoing,
				)?;
			}
			NodePacket::PeerNotify(rank, route_coord, peer_count, peer_distance) => {
				// Record peer rank
				//let node_idx = self.index_by_session_id(session_id: &SessionID)
				//let session = self.remote_mut(return_node_idx)?.session_mut()?;
				self.remote_mut(return_node_idx)?
					.session_mut()?
					.direct_mut()?
					.record_peer_notify(rank);
				// Update remote
				self.action(NodeAction::UpdateRemote(
					return_node_id,
					Some(route_coord),
					peer_count,
					peer_distance,
				));
			}
			NodePacket::Traverse(ref traversal_packet) => {
				let closest_peer_idx = self.find_closest_peer(&traversal_packet.destination)?;
				let closest_peer = self.remote(closest_peer_idx)?;
				// Check if NodeEncryption is meant for this node
				if traversal_packet.encryption.is_for_node(&self) {
					if let Some(return_route_coord) = traversal_packet.origin {
						println!(
							"Node({}) Received encryption: {:?}",
							self.node_id, traversal_packet
						);
						// Respond to encryption and set return session type as traversal
						if let Some((node_idx, packet)) = self.parse_node_encryption(
							traversal_packet.clone().encryption,
							SessionType::traversed(return_route_coord),
							outgoing,
						)? {
							self.parse_node_packet(node_idx, packet, outgoing)?;
						}
					} else {
						log::info!(
							"Node({}) send message with no return coordinates: {:?}",
							return_node_id,
							traversal_packet.encryption
						);
					}
				} else {
					// Check if next node is not node that I received the packet from
					if return_node_id != closest_peer.node_id {
						self.send_packet(closest_peer_idx, received_packet, outgoing)?;
					} else if let Some(_origin) = traversal_packet.origin {
						// Else, try to traverse packet back to origin
						log::error!("Packet Was Returned back, there seems to be a packet loop");
						//unimplemented!("Implement Traversed Packet Error return")
						//self.send_packet(closest_peer, TraversedPacket::new(origin, NodeEncryption::Notify { }, None), outgoing)
					}
				}
			}
			NodePacket::Data(data) => {
				println!(
					"{} -> {}, Data: {}",
					return_node_id,
					self.node_id,
					String::from_utf8_lossy(&data)
				);
			} //_ => { }
		}
		Ok(())
	}

	/// Initiate handshake process and send packets when completed
	pub fn connect(
		&mut self,
		dest_node_id: NodeID,
		session_type: SessionType,
		initial_packets: Vec<NodePacket>,
		outgoing: &mut PacketVec,
	) -> Result<(), NodeError> {
		let session_id: SessionID = rand::random(); // Create random session ID
											//let self_node_id = self.node_id;
		let self_ticks = self.ticks;
		let self_node_id = self.node_id;
		let (_, remote) = self.add_remote(dest_node_id)?;

		remote.pending_session = Some(Box::new((
			session_id,
			self_ticks,
			initial_packets,
			session_type.clone(),
		)));

		let encryption = NodeEncryption::Handshake {
			recipient: dest_node_id,
			session_id,
			signer: self_node_id,
		};
		// TODO: actual cryptography
		match session_type {
			SessionType::Direct(direct) => {
				// Directly send
				outgoing.push(encryption.package(direct.net_addr));
			}
			SessionType::Traversed(traversal) => {
				// Send traversed through closest peer
				let self_route_coord = self.route_coord.ok_or(NodeError::NoCalculatedRouteCoord)?;
				let closest_peer = self.find_closest_peer(&traversal.route_coord)?;
				self.send_packet(
					closest_peer,
					TraversedPacket::new(traversal.route_coord, encryption, Some(self_route_coord)),
					outgoing,
				)?;
			}
			_ => unimplemented!(),
		}

		Ok(())
	}
	// Create multiple Routed Sessions that sequentially resolve their pending_route fields as Traversed Packets are acknowledged
	/* fn routed_connect(&mut self, dest_node_id: NodeID, outgoing: &mut PacketVec) {
		//let routed_session_id: SessionID = rand::random();

		let remote = self.add_remote(dest_node_id);
		let remote
		//remote.pending_session = Some((session_id, usize::MAX, initial_packets));
		let closest_node
	} */
	/// Parses handshakes, acknowledgments and sessions, Returns Some(remote_net_addr, packet_to_parse) if session or handshake finished
	fn parse_packet(
		&mut self,
		received_packet: InternetPacket,
		outgoing: &mut PacketVec,
	) -> Result<Option<(RemoteIdx, NodePacket)>, NodeError> {
		if received_packet.dest_addr != self.net_addr {
			return Err(NodeError::InvalidNetworkRecipient {
				from: received_packet.src_addr,
				intended_dest: received_packet.dest_addr,
			});
		}

		if let Some(request) = received_packet.request {
			match request {
				InternetRequest::RouteCoordDHTReadResponse(query_node_id, route_option) => {
					if let Some(query_route_coord) = route_option {
						let (_, remote) = self.add_remote(query_node_id)?;
						remote.route_coord.get_or_insert(query_route_coord);
					} else {
						log::warn!("No Route Coordinate found for: {:?}", query_node_id);
					}
				}
				InternetRequest::RouteCoordDHTWriteResponse(_) => {}
				_ => {
					log::warn!("Not a InternetRequest Response variant")
				}
			}
			return Ok(None);
		}

		let encryption = NodeEncryption::unpackage(&received_packet)?;
		self.parse_node_encryption(
			encryption,
			SessionType::direct(received_packet.src_addr),
			outgoing,
		)
	}
	fn parse_node_encryption(
		&mut self,
		encryption: NodeEncryption,
		return_session_type: SessionType,
		outgoing: &mut PacketVec,
	) -> Result<Option<(RemoteIdx, NodePacket)>, NodeError> {
		//log::trace!("Node({}) Received Node Encryption with return session {:?}: {:?}", self.node_id, return_session_type, encryption);

		let self_ticks = self.ticks;
		let self_node_id = self.node_id;
		Ok(match encryption {
			NodeEncryption::Handshake {
				recipient,
				session_id,
				signer,
			} => {
				if recipient != self.node_id {
					Err(RemoteError::UnknownAckRecipient { recipient })?;
				}
				let (remote_idx, remote) = self.add_remote(signer)?;
				// Check if there is not already a pending session
				if remote.pending_session.is_some() {
					if self_node_id < remote.node_id {
						remote.pending_session = None
					}
				}

				let mut session = RemoteSession::new(session_id, return_session_type);
				let return_ping_id = session.tracker.gen_ping(self_ticks);
				let acknowledgement = NodeEncryption::Acknowledge {
					session_id,
					acknowledger: recipient,
					return_ping_id,
				};
				let packet = session.gen_packet(acknowledgement, self)?;
				outgoing.push(packet);
				self.remote_mut(remote_idx)?.session = Some(session);

				self.sessions.insert(session_id, remote_idx);
				log::debug!(
					"[{: >6}] Node({:?}) Received Handshake: {:?}",
					self_ticks,
					self_node_id,
					encryption
				);
				None
			}
			NodeEncryption::Acknowledge {
				session_id,
				acknowledger,
				return_ping_id,
			} => {
				let remote_idx = self.index_by_node_id(&acknowledger)?;
				let mut remote = self.remote_mut(remote_idx)?;
				if let Some(boxed_pending) = remote.pending_session.take() {
					let (
						pending_session_id,
						time_sent_handshake,
						packets_to_send,
						pending_session_type,
					) = *boxed_pending;
					if pending_session_id == session_id {
						// Create session and acknowledge out-of-tracker ping
						let mut session = RemoteSession::new(session_id, pending_session_type);
						let ping_id = session.tracker.gen_ping(time_sent_handshake);
						let distance = session.tracker.acknowledge_ping(ping_id, self_ticks)?;
						remote.session = Some(session); // update remote

						// Update packets
						let packets_to_send =
							self.update_connection_packets(remote_idx, packets_to_send)?;

						// Send connection packets
						self.send_packet(
							remote_idx,
							NodePacket::ConnectionInit(return_ping_id, packets_to_send),
							outgoing,
						)?;
						// Make note of session
						self.sessions.insert(session_id, remote_idx);
						self.direct_sorted.insert(distance, remote_idx);
						self.route_map
							.add_edge(self.node_id, acknowledger, distance);

						log::debug!(
							"[{: >6}] Node({:?}) Received Acknowledgement: {:?}",
							self_ticks,
							self_node_id,
							encryption
						);
						None
					} else {
						Err(RemoteError::UnknownAck { passed: session_id })?
					}
				} else {
					Err(RemoteError::NoPendingHandshake)?
				}
			}
			NodeEncryption::Session { session_id, packet } => {
				Some((self.index_by_session_id(&session_id)?, packet))
			}
			_ => {
				unimplemented!();
			}
		})
	}
	fn update_connection_packets(
		&self,
		return_node_idx: RemoteIdx,
		packets: Vec<NodePacket>,
	) -> Result<Vec<NodePacket>, NodeError> {
		let distance = self.remote(return_node_idx)?.session()?.tracker.dist_avg;
		Ok(packets
			.into_iter()
			.map(|packet| match packet {
				NodePacket::ExchangeInfo(_, _, _) => {
					NodePacket::ExchangeInfo(self.route_coord, self.remotes.len(), distance)
				}
				_ => packet,
			})
			.collect::<Vec<NodePacket>>())
	}
	fn send_packet(
		&self,
		node_idx: RemoteIdx,
		packet: NodePacket,
		outgoing: &mut PacketVec,
	) -> Result<(), NodeError> {
		let remote = self.remote(node_idx)?;
		let packet = remote.gen_packet(packet, self)?;
		outgoing.push(packet);
		Ok(())
	}
	fn calculate_route_coord(&mut self) -> Result<RouteCoord, NodeError> {
		// TODO: THIS CODE IS TERRIBLE AND NOT FUTURE-PROOF, NEEDS REIMPLEMENTATION FOR 3 DIMENSIONS AND FIX PRECISION ISSUES
		struct NodeCircle {
			coord: Vector2<f64>,
			dist: f64,
			list_index: usize,
		}

		// Get 10 closest nodes
		use itertools::Itertools;
		let closest_nodes = self.direct_sorted.iter().enumerate().filter_map(|(idx, (_,node_idx))| {
			let result: anyhow::Result<NodeCircle> = try {
				let node = self.remote(*node_idx)?;
				NodeCircle {
					coord: node.route_coord.ok_or(NodeError::NoCalculatedRouteCoord)?.map(|s|s as f64).coords,
					dist: node.session()?.tracker.dist_avg as f64,
					list_index: idx,
				}
			};
			result.ok()
		}).take(10).collect::<Vec<NodeCircle>>();

		let points = closest_nodes.iter().tuple_combinations().filter_map(|(node_a, node_b)| {
			let result: anyhow::Result<Vector2<f64>> = try {
				// Algorithm from: https://www.desmos.com/calculator/9mkzwevrns and https://math.stackexchange.com/questions/256100/how-can-i-find-the-points-at-which-two-circles-intersect
				let dist = node_a.coord.metric_distance(&node_b.coord);
				//let dist = nalgebra::distance(&node_a.coord, &circle_b.coord); // Distance
				let rad_a_sq = (node_a.dist * node_a.dist) as f64; // Radius of Circle A Squared
				let rad_b_sq = (node_b.dist * node_b.dist) as f64; // Radius of Circle B SQuared
				let dist_sq = dist * dist;

				let a = (rad_a_sq - rad_b_sq) / (2.0 * dist_sq);
				let middle = (node_a.coord + node_b.coord) / 2.0  + a * (node_b.coord - node_a.coord);

				let c = f64::sqrt( (2.0 * (rad_a_sq + rad_b_sq) / dist_sq) - ((rad_a_sq - rad_b_sq).powi(2) / (dist_sq * dist_sq) ) - 1.0);
				let offset = c * Vector2::new(node_a.coord.y + node_b.coord.y, node_a.coord.x - node_b.coord.x,) / 2.0;

				let intersection_1 = middle + offset;
				let intersection_2 = middle - offset;

				let intersection_points = closest_nodes.iter().filter(|&node|node.list_index != node_a.list_index || node.list_index != node_b.list_index)
					.map(|s|{
						let dist_intersect_1 = (intersection_1 - s.coord).magnitude() - s.dist;
						let dist_intersect_2 = (intersection_2 - s.coord).magnitude() - s.dist;
						if dist_intersect_1 < dist_intersect_2 { intersection_1 } else { intersection_2 }
					}).collect::<Vec<Vector2<f64>>>();
				// Calculate Average
				intersection_points.iter().fold(Vector2::new(0.0,0.0), |acc, &x| acc + x) / intersection_points.len() as f64
			};
			result.ok()
		}).collect::<Vec<Vector2<f64>>>();
		let average_point = points.iter().fold(Vector2::new(0.0,0.0), |acc, &x| acc + x) / points.len() as f64;
		let average_point = average_point.map(|s|s as i64);
		Ok(Point::from(average_point))
	}
}

impl fmt::Display for Node {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Node {}, /net/{}", self.node_id, self.net_addr)?;
		if let Some(route_coord) = self.route_coord {
			write!(f, ", @ ({}, {})", route_coord.x, route_coord.y)?;
		}
		for (_, remote) in self.remotes.iter() {
			writeln!(f)?;
			if let Ok(session) = remote.session() {
				let session_type_char = match &session.session_type {
					SessionType::Direct(direct) => match direct.peer_status.bits() {
						1 => ">",
						2 => "<",
						3 => "=",
						_ => ".",
					},
					SessionType::Traversed(_) => "~",
					SessionType::Routed(_) => "&",
				};
				write!(f, " {} | NodeID({})", session_type_char, remote.node_id)?;
				match &session.session_type {
					SessionType::Direct(direct) => write!(f, ", /net/{}", direct.net_addr)?,
					SessionType::Traversed(traversed) => write!(
						f,
						", @ ({}, {})",
						traversed.route_coord.x, traversed.route_coord.y
					)?,
					SessionType::Routed(routed) => {
						write!(
							f,
							", @ ({}, {}): ",
							routed.route_coord.x, routed.route_coord.y
						)?;
						for node_id in &routed.proxy_nodes {
							write!(f, "{} -> ", node_id)?;
						}
						write!(f, "{}", remote.node_id)?;
					}
				}
				write!(f, ", s:{}", session.session_id)?;
			} else {
				write!(f, "   | NodeID({})", remote.node_id)?;
				if let Some(route_coord) = remote.route_coord {
					write!(f, ", @? ({}, {})", route_coord.x, route_coord.y)?;
				}
			}
		}
		writeln!(f)
	}
}
fn node_id_formatter(node: &Option<Node>, f: &mut fmt::Formatter) -> fmt::Result {
	if let Some(node) = node {
		write!(f, "Node({})", node.node_id)
	} else {
		write!(f, "None")
	}
	
}