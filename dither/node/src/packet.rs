

use bytecheck::CheckBytes;
use rkyv::{Archive, Archived, Deserialize, Infallible, Serialize};

use crate::{net::Network, session::SessionKey};
use super::{NodeID, RouteCoord};
/// Packets that are sent between nodes in this protocol.
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[archive(bound(serialize = "__S: rkyv::ser::ScratchSpace + rkyv::ser::Serializer"))]
#[archive_attr(derive(CheckBytes))]
pub enum NodePacket<Net: Network> {
	/// Initiating Packet with unknown node
	InitUnknown { initiating_id: NodeID },
	/// Response to InitUnknown packet, Init packet might be sent after this
	InitAckUnknown { acknowledging_id: NodeID },
	/// Initial Packet, establishes encryption as well as some other things
	Init {
		initiating_id: NodeID,
		init_session_key: SessionKey,
		receiving_id: NodeID, // In future, Init packet will be asymmetrically encrypted with remote public key
	},

	/// Response to the Initial Packet, establishes encrypted tunnel.
	InitAck {
		ack_session_key: SessionKey, // Session key sent by Init, acknowledged
		acknowledging_id: NodeID,    // Previously receiving_id in Init packet
		receiving_id: NodeID,        // Previously initiating_id in Init packet
	},
	/// Sent back if received non-encrypted, non-init packet
	BadPacket {
		#[omit_bounds] packet: Box<NodePacket<Net>>,
	},

	/// Packet representing encryption
	Session {
		session_key: SessionKey,
		#[omit_bounds] encrypted_packet: Box<NodePacket<Net>>,
	},
	/// Traversing packet
	Traversal {
		/// Place to Route Packet to
		destination: RouteCoord,
		/// Packet to traverse to destination node
		#[omit_bounds] session_packet: Box<NodePacket<Net>>, // Must be type Init or Session packet
	},
	/// Packet representing an origin location
	Return {
		#[omit_bounds] packet: Box<NodePacket<Net>>,
		origin: RouteCoord,
	},

	/// ### Connection System
	/// Sent immediately after establishing encrypted session, allows other node to get a rough idea about the node's latency
	/// Contains list of packets for remote to respond to
	ConnectionInit {
		ping_id: u128,
		#[omit_bounds] initial_packets: Vec<NodePacket<Net>>,
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
	WantPing(NodeID, Net::Addr),
	/// Sent when node accepts a WantPing Request
	/// * `NodeID`: NodeID of Node who send the request in response to a RequestPings
	/// * `u64`: Distance to that nodeTraversedPacket
	AcceptWantPing(NodeID, u64),

	/* /// Request a session that is routed through node to another RouteCoordinate
	RoutedSessionRequest(RouteCoord),
	RoutedSessionAccept(), */
	/// Raw Data Packet
	Data(Vec<u8>),
}
impl<Net: Network> NodePacket<Net> 
where <Net::Addr as Archive>::Archived: Deserialize<Net::Addr, Infallible>
{
	pub fn from_archive(archive: &Archived<NodePacket<Net>>) -> Self
	{
		Deserialize::<NodePacket<Net>, Infallible>::deserialize(archive, &mut Infallible).unwrap()
	}
}