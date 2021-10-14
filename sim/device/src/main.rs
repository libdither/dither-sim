//! Standalone executable for dither-core to be run by simulation. Commands sent via stdin/stdout

use std::{fmt::Display, net::Ipv4Addr, str::FromStr};

use serde::{Serialize, Deserialize};

use dither_core as core;

#[derive(Debug, Serialize, Deserialize)]
pub enum DeviceCommand {
	Connect(Ipv4Addr, u16),
}
impl Display for DeviceCommand {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let string = ron::to_string(self).expect("DeviceCommand should be serializable");
		f.write_str(&string)
	}
}
impl FromStr for DeviceCommand {
	type Err = ron::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		ron::from_str(s)
	}
}

fn main() {
	let core = 
}

