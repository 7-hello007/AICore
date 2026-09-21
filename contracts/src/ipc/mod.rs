pub mod protocol;

pub use protocol::{
    Envelope, IpcError, Request, ResponseData, ResponseEnvelope, IPC_PROTOCOL_VERSION,
};
