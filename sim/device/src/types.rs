use serde::{Serialize, Deserialize};
use std::{fmt::Display, str::FromStr};

use libdither::commands::{DitherCommand, DitherEvent};

#[derive(Debug, Serialize, Deserialize)]
pub enum DeviceCommand {
	DitherCommand(DitherCommand),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DeviceEvent {
	DitherEvent(DitherEvent),
	Debug(String),
	Error(String),
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

impl Display for DeviceEvent {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let string = ron::to_string(self).expect("DeviceEvent should be serializable");
		f.write_str(&string)
	}
}
impl FromStr for DeviceEvent {
	type Err = ron::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		ron::from_str(s)
	}
}