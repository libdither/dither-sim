use tokio_util::{Encoder, Decoder};
use rkyv::Archive;

struct RkyvCodec<T: Archive>;

impl Encoder for RkyvCodec {

}

impl Decoder for RkyvCodec {
	
}