//#![allow(dead_code)]

use std::{collections::HashMap, fmt::Debug, fs::File, hash::Hash, io::{BufRead, BufReader, Write}};
use std::any::Any;
use std::ops::Range;

use anyhow::Context;
use rand::Rng;
use serde::{Deserialize, de::DeserializeOwned};
use smallvec::SmallVec;

mod router;
use router::NetSimRouter;

use crate::{Node, node::RouteCoord};

pub const FIELD_DIMENSIONS: (Range<i32>, Range<i32>) = (-320..320, -130..130);

#[derive(Error, Debug)]
pub enum InternetError {
	#[error("There is no node for this NetAddr: {net_addr}")]
	NoNodeError { net_addr: NetAddr },
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}

#[derive(Debug)]
pub enum NetSimRequest<CN: CustomNode + ?Sized> {
	RouteCoordDHTRead(CN::CustomNodeUUID),
	RouteCoordDHTWrite(CN::CustomNodeUUID, RouteCoord),
	RouteCoordDHTReadResponse(CN::CustomNodeUUID, Option<RouteCoord>),
	RouteCoordDHTWriteResponse(Option<(CN::CustomNodeUUID, RouteCoord)>),
	RandomNodeRequest(u32),
	RandomNodeResponse(u32, Option<CN::CustomNodeUUID>),
}

#[derive(Default, Debug)]
pub struct NetSimPacket<CN: CustomNode + ?Sized> {
	pub dest_addr: NetAddr,
	pub data: Vec<u8>,
	pub src_addr: NetAddr,
	pub request: Option<NetSimRequest<CN>>,
}
impl<CN: CustomNode> NetSimPacket<CN> {
	pub fn gen_request(dest_addr: NetAddr, request: NetSimRequest<CN>) -> Self { Self { dest_addr, data: vec![], src_addr: dest_addr, request: Some(request) } }
}

pub type NetAddr = u128;
pub type NetSimPacketVec<CN> = SmallVec<[NetSimPacket<CN>; 32]>;

pub trait CustomNode: Debug + Default {
	type CustomNodeAction;
	type CustomNodeUUID: Debug + Hash + Eq + Clone + serde::Serialize + DeserializeOwned;
	fn net_addr(&self) -> NetAddr;
	fn unique_id(&self) -> Self::CustomNodeUUID;
	fn tick(&mut self, incoming: NetSimPacketVec<Self>) -> NetSimPacketVec<Self>;
	fn action(&mut self, action: Self::CustomNodeAction);
	fn as_any(&self) -> &dyn Any;
	fn set_deus_ex_data(&mut self, data: Option<RouteCoord>);
}


#[derive(Debug, Serialize, Deserialize)]
pub struct NetSim<CN: CustomNode> {
	pub nodes: HashMap<NetAddr, CN>,
	pub router: NetSimRouter<CN>,
	route_coord_dht: HashMap<CN::CustomNodeUUID, RouteCoord>,
}
impl<CN: CustomNode> NetSim<CN> {
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