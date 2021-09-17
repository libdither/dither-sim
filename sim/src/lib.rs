#[macro_use]
extern crate serde;
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate derivative;
/* #[macro_use]
extern crate bitflags; */
//#![allow(dead_code)]

use std::{collections::HashMap, fmt::Debug, fs::File, io::{BufReader, Write}};
use std::ops::Range;

use anyhow::Context;
use nalgebra::Vector2;

use serde::Deserialize;


use node::{Node, RouteCoord, net};

/// All Dither Nodes and Routing Nodes will be organized on a field
/// Internet Simulation Field Dimensions (Measured in Nanolightseconds): 64ms x 26ms
pub const FIELD_DIMENSIONS: (Range<i32>, Range<i32>) = (-320000..320000, -130000..130000);

pub type FieldPosition = Vector2<i32>;
pub type Latency = u64;

/// Internet node type, has direct peer-to-peer connections and maintains a routing table to pick which direction a packet goes down.
#[derive(Derivative)]
#[derivative(Debug)]
struct InternetNode {
	/// Position of this Node in the Internet Simulation Field
	position: FieldPosition,
	/// Internal latency of this node's internal network and packet processing (Measure in nanoseconds)
	internal_latency: Latency,
	/// Whether this node contains the Dither Core Service
	node: Option<Node>,
}
impl InternetNode {
	pub fn new(position: FieldPosition, internal_latency: Latency) -> Self {
		Self {
			position,
			internal_latency,
			node: None,
		}
	}
}

#[derive(Error, Debug)]
pub enum InternetError {
	#[error("There is no node for this NetAddr: {net_addr:?}")]
	NoNodeError { net_addr: net::Address },
	#[error(transparent)]
	Other(#[from] anyhow::Error),
}


#[derive(Debug, Serialize, Deserialize)]
pub struct Internet {
	netsim: netsim_embed::Netsim<>,
	route_coord_dht: HashMap<node::NodeID, RouteCoord>,
}
impl Internet {
	pub fn new() -> Internet {
		Internet {
			netsim: NetSim::new(),
			route_coord_dht: HashMap::new(),
		}
	}
}
impl Internet {
	pub fn spawn(position: FieldPosition, internal_latency: Latency) {
		
		let machine = netsim_embed::Machine::new(id, plug, cmd).await;
		
	}
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