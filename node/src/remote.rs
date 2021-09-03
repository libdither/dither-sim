//! This is the remote module, It manages actions too and from a remote node

use std::{sync::Arc, time::Instant};

use crate::{Remote, net::Connection, packet::NodePacket, session};

use super::{Node, NodeError, NodeID, NodeAction, RouteCoord};
use session::*;

use async_std::channel::{self, Receiver, Sender};
use thiserror::Error;

/// Actions received by the task managing a connection to a remote node from the main node thread.
pub enum RemoteAction {
	/// Receive Route Coordinate Query
	QueryRouteCoordResponse(RouteCoord),
}

#[derive(Error, Debug)]
pub enum RemoteNodeError {
    #[error("There is no active session with the node: {node_id:?}")]
	NoSessionError { node_id: NodeID },
	#[error("Received Acknowledgement even though there are no pending handshake requests")]
	NoPendingHandshake,
	#[error("Session Error")]
	SessionError(#[from] SessionError),
}

/// Remote Node Is an Internal Structure of a Dither Node, it is managed by an independent thread when the remote is connected and sends messages back and forth with the session and the main node.
/// The 
#[derive(Debug)]
pub struct RemoteNode {
	/// The ID of the remote node, This structure is created when an encrypted link is established.
	node_id: Option<NodeID>,

	/// Connection Object
	connection: Arc<Connection>,

	/// Known Route Coordinate to communicate with remote node.
	route_coord: Option<RouteCoord>,

	// Action receivers and senders
	action_receiver: Receiver<RemoteAction>,
	action_sender: Sender<RemoteAction>,
}
impl RemoteNode {
	pub fn new_known_remote(node_id: Option<NodeID>, connection: Connection) -> (RemoteNode, Remote) {
		let (action_sender, action_receiver) = channel::bounded(20);
		(Self {
			node_id,
			connection,
			route_coord: None,
			action_receiver,
			action_sender,
		}, Remote {
			node_id,
			address: connection.address,
			action_sender,
		})
	}
	pub fn new(connection: Connection) -> (RemoteNode, Remote) {
		Self::new_known_remote(None, connection)
	}
	// Run remote action event loop. Consumes itself, should be run on independent thread
	pub async fn run(self, node_action: Sender<NodeAction>) {
		let node_action = node_action;

		let (join_handle, session_action) = session::Session::start(self.connection.clone(), self.action_sender);
		while let Ok(action) = self.action_receiver.recv().await {

		}
	}

	fn session_active(&self) -> bool {
		self.session.is_some() && self.pending_session.is_none()
	}
	/// Check if a peer is viable or not
	// TODO: Create condition that rejects nodes if there is another closer node located in a specific direction
	fn is_viable_peer(&self, _self_route_coord: RouteCoord) -> Option<RouteCoord> {
		if let (Some(route_coord), Some(session)) = (self.route_coord, &self.session) {
			//let avg_dist = session.tracker.dist_avg;
			//let route_dist = nalgebra::distance(route_coord.map(|s|s as f64), self_route_coord.map(|s|s as f64));
			if session.direct().is_ok() {
				return Some(route_coord.clone());
			} else { None }
		} else { None }
	}
}