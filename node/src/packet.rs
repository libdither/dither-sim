
use crate::{net, session::SessionKey};

use super::{net::Address, NodeError, NodeID, RouteCoord};

/// Packets that are sent between nodes in this protocol.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NodePacket {
	/// Initiating Packet with unknown node
	InitUnknown {
		initiating_id: NodeID,
	},
	/// Response to InitUnknown packet, Init packet might be sent after this
	InitAckUnknown {
		acknowledging_id: NodeID,
	},
	/// Initial Packet, establishes encryption as well as some other things
	Init {
		initiating_id: NodeID,
		init_session_key: SessionKey,
		receiving_id: NodeID, // In future, Init packet will be asymmetrically encrypted with remote public key
	},
	
	/// Response to the Initial Packet, establishes encrypted tunnel.
	InitAck {
		ack_session_key: SessionKey, // Session key sent by Init, acknowledged
		acknowledging_id: NodeID, // Previously receiving_id in Init packet
		receiving_id: NodeID, // Previously initiating_id in Init packet
	},
	/// All Packets that are not Init-type should be wrapped in session encryption
	Session {
		session_key: SessionKey,
		encrypted_packet: Box<NodePacket>,
	},
	Traversal {
		/// Place to Route Packet to
		destination: RouteCoord,
		/// Packet to traverse to destination node
		session_packet: Box<NodePacket>, // Must be type Init-type, or Session
		/// Signed & Assymetrically encrypted return location
		origin: Option<RouteCoord>,
	},

	/// ### Connection System
	/// Sent immediately after establishing encrypted session, allows other node to get a rough idea about the node's latency
	/// Contains list of packets for remote to respond to 
	ConnectionInit {
		ping_id: u128,
		initial_packets: Vec<NodePacket>,
	},

	/// Exchange Info with another node
	ExchangeInfo {
		/// Tell another node my Route Coordinate if I have it
		calculated_route_coord: Option<RouteCoord>,
		/// Number of direct connections I have
		useful_connections: usize, 
		/// ping (latency) to remote node
		average_latency: u64, 
		latency_accuracy: u32,
		response: bool,
	}, 

	/// Notify another node of peership
	/// * `usize`: Rank of remote in peer list
	/// * `RouteCoord`: My Route Coordinate
	/// * `usize`: Number of peers I have
	/// * `u64`: Latency to remote node
	PeerNotify(usize, RouteCoord, usize, u64),
	/// Propose routing coordinates if nobody has any nodes
	ProposeRouteCoords(RouteCoord, RouteCoord), // First route coord = other node, second route coord = myself
	/// Proposed route coords (original coordinates, orientation, bool), bool = true if acceptable
	ProposeRouteCoordsResponse(RouteCoord, RouteCoord, bool), 

	/// ### Self-Organization System
	/// Request a certain number of another node's peers that are closest to this node to make themselves known
	/// * `usize`: Number of peers requested
	/// * `Option<RouteCoord>`: Route Coordinates of the other node if it has one
	RequestPings(usize, Option<RouteCoord>),

	/// Tell a peer that this node wants a ping (implying a potential direct connection)
	WantPing(NodeID, net::Address),
	/// Sent when node accepts a WantPing Request
	/// * `NodeID`: NodeID of Node who send the request in response to a RequestPings
	/// * `u64`: Distance to that nodeTraversedPacket
	AcceptWantPing(NodeID, u64),

	/* /// Request a session that is routed through node to another RouteCoordinate
	RoutedSessionRequest(RouteCoord),
	RoutedSessionAccept(), */

	/// Raw Data Packet
	Data(Vec<u8>)
}