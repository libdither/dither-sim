use std::{net::Ipv4Addr, time::Duration};

use async_std::task::JoinHandle;
use device::{Address, DeviceCommand, DeviceEvent};
use nalgebra::Vector2;
use netsim_embed::{Ipv4Range, Machine, Network, Plug};
use node::{NodeID, RouteCoord};

use super::netsim_ext::WireHandle;

pub type FieldPosition = Vector2<i32>;
/// Measured in milliseconds
pub type Latency = u64;

pub enum InternetMachineConnection {
	Unconnected(Plug),
	Connected(Ipv4Addr),
}

pub struct InternetMachine {
	pub machine: Machine<DeviceCommand, DeviceEvent>,
	pub event_join_handle: JoinHandle<()>,
	pub connection_status: InternetMachineConnection,
	/// Wire for internal latency simulation
	pub internal_wire: WireHandle, 
	pub internal_latency: Latency,
}
impl InternetMachine {
	pub async fn set_latency(&mut self, latency: Latency) {
		self.internal_latency = latency;
		self.internal_wire.set_delay(Duration::from_millis(self.internal_latency)).await;
	}
	pub fn latency(&self) -> Latency {
		self.internal_latency
	}
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
	pub position: FieldPosition,
	pub internal_latency: Latency,
	pub local_address: Option<Ipv4Addr>,
	pub node_type: NodeType,
}
#[derive(Debug, Clone)]
pub enum NodeType {
	Network,
	Machine,
}
#[derive(Debug, Clone)]
pub struct MachineInfo {
	pub route_coord: RouteCoord,
	pub listening_addr: Address,
	pub public_addr: Address,
	pub node_id: NodeID,
}
#[derive(Debug, Clone)]
pub struct NetworkInfo {
	pub connections: Vec<usize>,
	pub ip_range: Ipv4Range,
}

enum NodeVariant {
	Network(Network),
	Machine(InternetMachine),
}

/// Internet node type, has direct peer-to-peer connections and maintains a routing table to pick which direction a packet goes down.
pub struct InternetNode {
	variant: NodeVariant,
	/// Position of this Node in the Internet Simulation Field
	position: FieldPosition,
}

impl InternetNode {
	pub fn from_machine(machine: InternetMachine, position: FieldPosition) -> Self {
		Self {
			variant: NodeVariant::Machine(machine),
			position,
		}
	}
	pub fn from_network(network: Network, position: FieldPosition) -> Self {
		Self {
			variant: NodeVariant::Network(network),
			position,
		}
	}
	pub fn gen_node_info(&self) -> NodeInfo {
		let (internal_latency, local_address, node_type) = match &self.variant {
			NodeVariant::Network(network) => {
				(Latency::MIN, Some(network.range().base_addr()), NodeType::Network)
			},
			NodeVariant::Machine(machine) => {
				(machine.latency(), match machine.connection_status {
					InternetMachineConnection::Connected(addr) => Some(addr),
					InternetMachineConnection::Unconnected(_) => None,
				}, NodeType::Machine)
			},
		};
		NodeInfo {
			position: self.position.clone(),
			internal_latency,
			local_address,
			node_type,
		}
	}
}

