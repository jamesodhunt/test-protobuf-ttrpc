// Description: Common types used by the client and server

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Config {
    pub server_uri: String,
    pub interactive: bool,
    pub force_abstract_socket: bool,

    // If true, use the "vsock" crate, else use the "nix" crate to handle vsock
    // client comms.
    pub use_vsock_crate_for_vsock: bool,

    pub tx: Option<Sender<bool>>,
}

#[derive(Debug, Clone)]
pub struct HelloService {
    pub cfg: Arc<Mutex<Config>>,
}
