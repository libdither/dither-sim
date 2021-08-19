/// Defines all the generic components of a node interacting with an internet structure.
/// A Node should be able to work in any kind of network. simulated or not. This file provides the basic structures that any network implementation will use to interact with a Node.

use async_std::io::{Read, Write};

/// Address that allows a Node to connect to another Node over a network implementation. This might be an IP address, a multiaddr, or just a number.
pub struct Address(Vec<u8>);

/// Represents a 2-way asyncronous stream of bytes and the address used to establish the connection.
pub struct Connection {
	pub address: Address,
	pub stream: Box<dyn Read + Write>,
}