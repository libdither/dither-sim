//! This is the remote module, It manages actions too and from a remote node

use crate::{Remote, net::Network, packet::NodePacket, session};

use super::{NodeID, NodeAction, RouteCoord};
use async_std::task::JoinHandle;
use futures::{SinkExt, StreamExt, channel::mpsc::{Receiver, SendError, Sender}};
use session::*;

use thiserror::Error;

/// Actions received by the task managing a connection to a remote node from the main node thread.
#[derive(Debug)]
pub enum RemoteAction<Net: Network> {
	/// From Main Thread
	/// Handle Connection passed through main node from network
	HandleConnection(Net::Addr, Net::Conn),
	/// Query Route Coord from Route Coord Lookup (see NetAction)
	RouteCoordQuery(RouteCoord),

	/// From Session Thread
	ReceivePacket(NodePacket<Net>),

	SessionError(Box<SessionError>),
}

#[derive(Error, Debug)]
pub enum RemoteNodeError {
    #[error("There is no active session with the node: {node_id:?}")]
	NoSessionError { node_id: NodeID },
	#[error("Received Acknowledgement even though there are no pending handshake requests")]
	NoPendingHandshake,
	#[error("Session Error")]
	SessionError(#[from] SessionError),
	#[error("Channel Send Error")]
	SessionChannelError(#[from] SendError),
}

/// Remote Node Is an Internal Structure of a Dither Node, it is managed by an independent thread when the remote is connected and sends messages back and forth with the session and the main node.
/// The 
#[derive(Debug)]
pub struct RemoteNode<Net: Network> {
	/// The ID of the remote node, Set when the NodeID is known beforehand or an encrypted link has just been connected
	node_id: Option<NodeID>,

	/// Known Route Coordinate to communicate with remote node.
	route_coord: Option<RouteCoord>,

	/// Current encrypted channel to remote
	session: Option<Session<Net>>,

	action_sender: Sender<RemoteAction<Net>>,
}
impl<Net: Network> RemoteNode<Net> {
	pub fn new<'a>(remote: Option<&'a Remote<Net>>, action_sender: Sender<RemoteAction<Net>>) -> RemoteNode<Net> {
		Self {
			node_id: remote.map(|r|r.node_id.clone()).flatten(),
			route_coord: remote.map(|r|r.route_coord.clone()).flatten(),
			session: remote.map(|r|r.session.clone()).flatten(),
			action_sender,
		}
	}
	// Run remote action event loop. Consumes itself, should be run on independent thread
	pub async fn run(mut self, mut action_receiver: Receiver<RemoteAction<Net>>, node_action: Sender<NodeAction<Net>>) -> Result<(), RemoteNodeError> {
		// TODO: Do return sending
		let _node_action = node_action;
		
		let (mut session_join_handle, mut session_action) = (None::<JoinHandle<Session<Net>>>, None::<Sender<SessionAction<Net>>>);
		while let Some(action) = action_receiver.next().await {
			match action {
				RemoteAction::HandleConnection(address, connection) => {
					(session_join_handle, session_action) = match session_action.clone() {
						Some(mut session_action) => {
							session_action.send(SessionAction::NewConnection(connection)).await?;
							(session_join_handle, Some(session_action))
						},
						None => {
							let session = self.session.take().unwrap_or(Session::new());
							Some(session.start(address, connection, self.action_sender.clone())).unzip()
						},
					}
				},
				RemoteAction::ReceivePacket(packet) => {
					match packet {
						_ => { todo!() }
					}
				},
				_ => { todo!() }
			}
		}
		// Wait for Session to end
		if let Some(join_handle) = session_join_handle {
			self.session = Some(join_handle.await);
		}
		Ok(())
	}
}