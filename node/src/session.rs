//! This session module manages the ongoing state of a connection to a remote node. It deals with encryption and packet parsing.
//! It has two "threads" that manage reading and writing, and both report back to the RemoteNode via RemoteActions

use async_std::{channel::{self, Sender}, task::{self, JoinHandle}};
use async_std::io::ReadExt;

use crate::{NodeAction, net::{Address, Connection}, packet::NodePacket, remote::RemoteAction};

pub type SessionKey = u128;

pub enum SessionAction {
	SendPacket(NodePacket),
	DecryptedPacket(NodePacket),
}
pub enum SessionError {
	TunnelClosed,
}

pub struct Session {
	key: SessionKey,
	net_addr: Address,
}
impl Session {
	pub fn start(connection: Connection, remote_action: Sender<RemoteAction>) -> (JoinHandle<()>, Sender<SessionAction>) {
		let (action_sender, action_receiver) = channel::bounded::<SessionAction>(20);

		let (reader, writer) = connection.stream;
		// Writing Thread, Listens to action_receiver and occasionally writes to writer
		let join_handle = task::spawn(async move {

			// Reading Thread
			let join_handle = task::spawn(async move {
				while let Ok(action) = action_receiver.recv().await {
					match action {
						SessionAction::SendPacket(packet) => {

						}
						_ => { log::error!("Session Received wrong action: {:?}", action) }
					}
				}
				reader.read_to_end().await

				//bincode::deserialize_slice()
				//action_sender
			});


			join_handle.await; // Waits for internal thread to complete
		});
		
		// Returns Join Handle and method of
		(join_handle, action_sender)
	}
}