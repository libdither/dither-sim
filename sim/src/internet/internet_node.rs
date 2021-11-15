use std::time::Duration;

use async_std::task::JoinHandle;
use device::{DeviceCommand, DeviceEvent};
use nalgebra::Vector2;
use netsim_embed::{Machine, Network, Plug};

use super::netsim_ext::{Wire, WireHandle};

pub type FieldPosition = Vector2<i32>;
/// Measured in milliseconds
pub type Latency = u64;

pub struct InternetMachine {
	pub machine: Machine<DeviceCommand, DeviceEvent>,
	pub event_join_handle: JoinHandle<()>,
	pub unconnected_plug: Option<Plug>,
	pub internal_wire: WireHandle,
}
impl InternetMachine {
	pub async fn set_latency(&mut self, latency: Latency) {
		self.internal_wire.set_delay(Duration::from_millis(latency)).await;
	}
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
	pub async fn from_machine(machine: InternetMachine, position: FieldPosition) -> Self {
		Self {
			variant: NodeVariant::Machine(machine),
			position,
		}
	}
	pub async fn from_network(network: Network, position: FieldPosition) -> Self {
		Self {
			variant: NodeVariant::Network(network),
			position,
		}
	}
}

