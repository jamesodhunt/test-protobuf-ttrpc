// Description: ttrpc logic entry point

use slog::{o, Logger};

use crate::client::client;
use crate::server::server;
use crate::types::{Config, HelloService};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

pub type Result<T> = std::result::Result<T, String>;

pub fn run_ttrpc(
    logger: &Logger,
    server_uri: &str,
    create_server: bool,
    interactive: bool,
    commands: Vec<&str>,
    force_abstract_socket: bool,
    use_vsock_crate_for_vsock: bool,
) -> Result<()> {
    let (tx, rx) = channel::<bool>();

    let c = Config {
        server_uri: server_uri.to_string(),
        interactive,
        force_abstract_socket,
        use_vsock_crate_for_vsock,
        tx: Some(tx),
    };

    let service = HelloService {
        cfg: Arc::new(Mutex::new(c)),
    };

    let ttrpc_type = if create_server { "server" } else { "client" };

    // Maintain the global logger for the duration of the ttrpc comms
    let _guard =
        slog_scope::set_global_logger(logger.new(o!("subsystem" => "ttrpc", "type" => ttrpc_type)));

    match create_server {
        true => server(&service, rx),
        false => client(&service, commands),
    }
}
