/// Defines all the generic components of a node interacting with an internet structure.
/// A Node should be able to work in any kind of network. simulated or not. This file provides the basic structures that any network implementation will use to interact with a Node.

use tokio::{io::{AsyncRead, AsyncWrite}, net::TcpStream};
//use futures::{AsyncBufRead, AsyncWrite};

use crate::{NodeID, RouteCoord};

/// Address that allows a Node to connect to another Node over a network implementation. This might be an IP address, a multiaddr, or just a number.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Archive, Serialize, Deserialize, serde::Serialize, serde::Deserialize)]
pub struct Address(Vec<u8>);

/// Represents a 2-way asyncronous stream of bytes and the address used to establish the connection.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Connection {
	/// Data object that allows an underlying protocol to route packets to another computer
	pub address: Address,
	/// Two-way Binary Stream object that connects losslessly to another remote peer
	#[derivative(Debug="ignore")]
	pub stream: TcpStream, // This will be a bytestream provided by the network implementation (i.e. libp2p)
}

/// Response Object sent wrapped in a NetAction when a connection is requested
#[derive(Debug)]
pub enum ConnectionResponse {
	/// Established Connection
	Established(Connection),
	/// Remote could not be located
	NotFound,
	/// Remote exists, but there was an error in establishing the connection. 
	Error(String),
}

/// Actions that can be sent to the Network Implementation (Most of these are temporary)
#[derive(Debug)]
pub enum NetAction {
	/// From Node
	/// Publish Route to "fake" DHT (will be replaced with real DHT kademlia DHT implementation in future)
	PublishRouteCoords(NodeID, RouteCoord),
	/// Query Route Coords from DHT
	QueryRouteCoord(NodeID),
	/// Establish a Connection to a remote
	Connect(Address),

	/// From Internet
	/// Response for QueryRouteCoord Action
	QueryRouteCoordResponse(NodeID, RouteCoord),
	/// Tell node about new address from network implementation.
	UpdateAddress(Address),
	/// Response for Connection
	ConnectResponse(ConnectionResponse),
	/// Handle Incoming Connection
	Incoming(Connection)
}