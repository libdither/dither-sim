
/// Multihash that uniquely identifying a node (represents the Multihash of the node's Public Key)
pub type NodeID = Vec<u8>;
/// Coordinate that represents a position of a node relative to other nodes in 2D space.
pub type RouteScalar = u64;

/// A location in the network for routing packets
//#[repr(transparent)]
pub type RouteCoord = (i64, i64);

/* impl RouteCoord {
	/// Get euclidian distance between two RouteCoords
	pub fn dist(self, other: &RouteCoord) -> f64 {
		let start_f64 = self.map(|s|s as f64);
		let end_f64 = other.map(|s|s as f64);
		nalgebra::distance(&start_f64, &end_f64)
	}
} */