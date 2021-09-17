//#![allow(dead_code)]

use std::{collections::HashMap, fmt::Debug, fs::File, hash::Hash, io::{BufRead, BufReader, Write}, sync::Arc};
use std::any::Any;
use std::ops::Range;

use anyhow::Context;
use nalgebra::Vector2;
use petgraph::graphmap::GraphMap;
use rand::Rng;
use serde::{Deserialize, de::DeserializeOwned};

mod router;
use router::NetSimRouter;

use node::{Node, RouteCoord};

/// All Dither Nodes and Routing Nodes will be organized on a field
/// Internet Simulation Field Dimensions (Measured in Nanolightseconds): 64ms x 26ms
pub const FIELD_DIMENSIONS: (Range<i32>, Range<i32>) = (-320000..320000, -130000..130000);

pub type FieldPosition = Vector2<i32>;
pub type Latency = u64;

/// Internet node type, has direct peer-to-peer connections and maintains a routing table to pick which direction a packet goes down.
#[derive(Derivative)]
#[derivative(Debug)]
struct NetNode {
	/// Position of this Node in the NetSim Simulation Field
	position: FieldPosition,
	/// Internal latency of this node's internal network and packet processing (Measure in nanoseconds)
	internal_latency: Latency,
	/// Whether this node contains the Dither Core Service
	node: Option<Node>,
	/// Connections to other nodes, set by network
	pub connections: Vec<Arc<NetWire>>,
}
impl NetNode {
	pub fn new(position: FieldPosition, internal_latency: Latency) -> Self {
		Self {
			position,
			internal_latency,
			node: None,
			connections: Vec::default(),
		}
	}
}

/// Internet Wire Type, provides delayed streams between NetNodes, set dynamically by NetSim object.
struct NetWire {
	latency: Latency,
}

/// Define actions that can be run by the simulation UI
#[derive(Clone, Debug)]
pub enum InternetEvent {
	/// Add node to Network
	AddNode(InternetNode),
}

#[derive(Error, Debug)]
pub enum InternetError {
	#[error("There is no node for this NetAddr: {net_addr}")]
	NoNodeError { net_addr: NetAddr },
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

#[derive(Debug)]
pub enum NetSimRequest {
	RouteCoordDHTRead(node::NodeID),
	RouteCoordDHTWrite(node::NodeID, RouteCoord),
	RouteCoordDHTReadResponse(node::NodeID, Option<RouteCoord>),
	RouteCoordDHTWriteResponse(Option<(node::NodeID, RouteCoord)>),
	RandomNodeRequest(u32),
	RandomNodeResponse(u32, Option<node::NodeID>),
}


#[derive(Debug, Serialize, Deserialize)]
pub struct NetSim {
	route_coord_dht: HashMap<node::NodeID, RouteCoord>,
}
impl NetSim {
	pub fn new() -> NetSim<CN> {
		NetSim {
			nodes: HashMap::new(),
			router: NetSimRouter::new(FIELD_DIMENSIONS),
			route_coord_dht: HashMap::new(),
		}
	}
	pub fn from_reader<CND: CustomNode + DeserializeOwned>(reader: impl BufRead) -> anyhow::Result<NetSim<CND>> {
		Ok(bincode::deserialize_from(reader)?)
	}
	pub fn lease(&self) -> NetAddr { self.nodes.len() as NetAddr }
	pub fn add_node(&mut self, node: CN, rng: &mut impl Rng) {
		self.router.add_node(node.net_addr(), rng);
		self.nodes.insert(node.net_addr(), node);
	}
	pub fn del_node(&mut self, net_addr: NetAddr) { self.nodes.remove(&net_addr); }
	pub fn node_mut(&mut self, net_addr: NetAddr) -> Result<&mut CN, InternetError> { self.nodes.get_mut(&net_addr).ok_or(InternetError::NoNodeError { net_addr }) }
	pub fn node(&self, net_addr: NetAddr) -> Result<&CN, InternetError> { self.nodes.get(&net_addr).ok_or(InternetError::NoNodeError { net_addr }) }
	pub fn tick(&mut self, ticks: usize, rng: &mut impl Rng) {
		//let packets_tmp = Vec::new();
		for _ in 0..ticks {
			for (&node_net_addr, node) in self.nodes.iter_mut() {
				// Get Packets going to node
				let incoming_packets = self.router.tick_node(node_net_addr);
				// Get packets coming from node
				let mut outgoing_packets = node.tick(incoming_packets);

				// Make outgoing packets have the correct return address or parse request
				for packet in &mut outgoing_packets {
					packet.src_addr = node_net_addr;
					if let Some(request) = &packet.request {
						log::debug!("NetAddr({:?}) Requested NetSimRequest::{:?}", node_net_addr, request);
						packet.request = Some(match *request {
							NetSimRequest::RouteCoordDHTRead(ref node_id) => {
								let node_id = node_id.clone();
								packet.dest_addr = packet.src_addr;
								let route = self.route_coord_dht.get(&node_id).map(|r|r.clone());
								NetSimRequest::RouteCoordDHTReadResponse(node_id, route)
							}
							NetSimRequest::RouteCoordDHTWrite(ref node_id, route_coord) => {
								packet.dest_addr = packet.src_addr;
								let old_route = self.route_coord_dht.insert(node_id.clone(), route_coord);
								NetSimRequest::RouteCoordDHTWriteResponse( old_route.map(|r|(node_id.clone(), r) ))
							}
							NetSimRequest::RandomNodeRequest(unique_id) => {
								use rand::prelude::IteratorRandom;
								let id = self.route_coord_dht.iter().choose(rng).map(|(id,_)|id.clone());
								NetSimRequest::RandomNodeResponse(unique_id, id)
							}
							_ => { log::error!("Invalid NetSimRequest variant"); unimplemented!() },
						});
					}
				}
				// Send packets through the router
				self.router.add_packets(outgoing_packets, rng);
				/* if let Some(rn) = self.router.node_map.get(&node_net_addr) {
					let cheat_coord = rn.position.clone().map(|s|s.floor() as i64);
					node.set_deus_ex_data( Some(cheat_coord) ) } */
			}
		}
	}
}
impl NetSim<Node> {
	pub fn save(&self, filepath: &str) -> Result<(), InternetError> {
		let mut file = File::create(filepath).context("failed to create file (check perms) at {}")?;
		let data = bincode::serialize(&self).context("failed to serialize network")?;
		file.write_all(&data).context("failed to write to file")?;
		Ok(())
	}
	pub fn load(&mut self, filepath: &str) -> Result<(), InternetError> {
		let file = File::open(filepath).context("failed to open file (check perms)")?;
		*self = bincode::deserialize_from(BufReader::new(file)).context("failed to deserialize network")?;
		Ok(())
	}
}