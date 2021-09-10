use std::marker::PhantomData;

use bytecheck::CheckBytes;
use tokio_util::codec::{Encoder, Decoder};
use rkyv::{Archive, Deserialize, Fallible, Infallible, Serialize, ser::{Serializer, serializers::{WriteSerializer}}, validation::validators::DefaultValidator, with::With};
use bytes::{Buf, BufMut, BytesMut};

const MAX_PACKET_LENGTH: usize = 1024 * 1024 * 16;

#[derive(Error, Debug)]
enum RkyvCodecError {
	#[error(transparent)]
	IoError(#[from] std::io::Error),
}

#[derive(Default)]
pub struct RkyvCodec<ItemType: Archive> {
	type_marker: PhantomData<ItemType>,
	/* serialize_marker: PhantomData<S>,
	deserialize_marker: PhantomData<D>, */
}
impl<ItemType: Archive> RkyvCodec<ItemType> {
	pub fn new() -> Self {
		Self {
			type_marker: Default::default(),
			/* serialize_marker: Default::default(),
			deserialize_marker: Default::default(), */
		}
	}
}

impl<ItemType: Archive + Serialize<WriteSerializer<bytes::buf::Writer<BytesMut>>>> Encoder<ItemType> for RkyvCodec<ItemType> {
	type Error = RkyvCodecError;

	fn encode(&mut self, item: ItemType, dst: &mut BytesMut) -> Result<(), Self::Error> {
		// Serialize Object
		let mut serializer = WriteSerializer::new((*dst).writer());
		serializer.pad(4); // Reserve space for length
		let item_len = serializer.serialize_value(&item).unwrap();
		dst = &mut serializer.into_inner().into_inner();
		
		// Don't send a string if it is longer than the other end will accept
		if item_len > MAX_PACKET_LENGTH {
			Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Frame of length {} is too large.", item_len)
			))?;
		}

		// Overwrite Length Bytes
		(&mut dst[0..4]).put_u32_le(item_len as u32);
		Ok(())
	}
}

impl<'a, ItemType: Archive + Deserialize<ItemType, Infallible>> Decoder for RkyvCodec<ItemType>
where
	<ItemType as Archive>::Archived: CheckBytes<DefaultValidator<'a>>,
{
	type Item = ItemType;
	type Error = std::io::Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error>
	where
		<ItemType as Archive>::Archived: CheckBytes<DefaultValidator<'a>>,
	{

		// Check if large enough to read u32.
		if src.len() < 4 {
			return Ok(None);
		}

		// Copy length marker from buffer, interpret as u32 and cast to usize
		let length = src.copy_to_bytes(4).get_u32_le() as usize;

		// Check that the length is not too large to avoid a denial of
		// service attack where the server runs out of memory.
		if length > MAX_PACKET_LENGTH {
			return Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Frame of length {} is too large.", length)
			));
		}
		
		// Check if full string has arrived yet
		if src.len() < (4 + length) {
			// Reserve space (helps with efficiency, not required)
			src.reserve(4 + length - src.len());

			// We inform the Framed that we need more bytes to form the next
			// frame.
			return Ok(None);
		}

		// interpret data from buffer as Archive, using validation feature to avoid unsafe code
		let archived = rkyv::check_archived_root::<ItemType>(&src[4..4 + length]).unwrap();
		// deserialize archive into an actual value
		let deserialized: With<ItemType, <ItemType as Archive>::Archived> = archived.deserialize(&mut Infallible).unwrap().into_inner();
		Ok(Some(deserialized.into_inner()))
	}
}