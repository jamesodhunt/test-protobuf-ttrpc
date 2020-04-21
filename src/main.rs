//--------------------------------------------------------------------
// Description: Noddy ttRPC client/server example/test program.
// Date: 2019-11-01
// Author: James Hunt <jamesodhunt@gmail.com>
//--------------------------------------------------------------------

use clap::{App, Arg, SubCommand};
use std::io;
use std::process::exit;

#[macro_use]
mod logging;
mod client;
mod server;
mod ttrpc;
mod types;

// Import the auto-generated modules
mod service;
mod service_ttrpc;

pub type Result<T> = std::result::Result<T, String>;

// XXX: Should really set from makefile
const VERSION: &str = "0.0.1";

fn show_usage_examples(program_name: &str) {
    const UNIX_URI: &str = "unix:///tmp/my.socket";
    const VSOCK_URI: &str = "vsock://-1";

    println!(
        r#"
Examples:

- Server:

  - Unix socket:

    $ {program} --server-uri {unix_uri:?} server

  - VSOCK socket:

    $ {program} --server-uri {unix_uri:?} server

- Clients:

  - Abstract Unix socket:

    - Non-interactive:

      $ {program} --server-uri {unix_uri:?} --abstract client \
          --commands "SayHello foo" \
          --commands "SayHello bar" \
          --commands "SayHello baz" \
          --commands Shutdown

    - Interactive:

      $ {program} --server-uri {unix_uri:?} --abstract --interactive client

  - Named Unix socket:

    - Non-interactive:

      $ {program} --server-uri {unix_uri:?} client \
          --commands "SayHello foo" \
          --commands "SayHello bar" \
          --commands "SayHello baz" \
          --commands Shutdown

    - Interactive:

      $ {program} --server-uri {unix_uri:?} --interactive client

  - VSOCK socket:

    - Non-interactive:

      $ {program} --server-uri {vsock_uri:?} client \
          --commands "SayHello foo" \
          --commands "SayHello bar" \
          --commands "SayHello baz" \
          --commands Shutdown

    - Interactive:

      $ {program} --server-uri {vsock_uri:?} --interactive client

    "#,
        program = program_name,
        unix_uri = UNIX_URI,
        vsock_uri = VSOCK_URI,
    );
}
fn real_main() -> Result<()> {
    let name = module_path!();

    let vsock_crate_names = &["vsock", "nix"];

    let app = App::new(name)
        .version(VERSION)
        .about("ttRPC client/server application.")
        .arg(
            Arg::with_name("abstract")
                .long("abstract")
                .help("Force use of an abstract socket"),
        )
        .arg(
            Arg::with_name("interactive")
                .short("i")
                .long("interactive")
                .help("Allow interactive client"),
        )
        .arg(
            Arg::with_name("server-uri")
                .long("server-uri")
                .help("server URI to use (unix:///some/where or vsock://cid:port)")
                .takes_value(true)
                .value_name("server-uri"),
        )
        .subcommand(
            SubCommand::with_name("client")
                .about("Create a ttRPC client")
                .arg(
                    Arg::with_name("commands")
                        .long("commands")
                        .takes_value(true)
                        .multiple(true)
                        .help("Commands to send to server"),
                )
                .arg(
                    Arg::with_name("crate-for-vsock")
                        .long("crate-for-vsock")
                        .takes_value(true)
                        .possible_values(vsock_crate_names)
                        .default_value("vsock")
                        .help("Specify which crate to use for vsock client comms"),
                ),
        )
        .subcommand(SubCommand::with_name("server").about("Create a ttRPC server"))
        .subcommand(SubCommand::with_name("help").about("Show examples"));

    let args = app.get_matches();

    let mut server: bool = false;

    let mut use_vsock_crate_for_vsock = false;

    let interactive = args.is_present("interactive");
    let abstract_socket = args.is_present("abstract");

    let mut commands: Vec<&str> = Vec::new();

    if let Some(args) = args.subcommand_matches("client") {
        if !interactive {
            commands = match args.values_of("commands") {
                Some(c) => c.collect(),
                None => return Err("need commands to send to server".to_string()),
            };
        }

        use_vsock_crate_for_vsock = match args.value_of("crate-for-vsock") {
            Some("vsock") => true,
            _ => false,
        };
    } else if let Some(_) = args.subcommand_matches("server") {
        server = true;
    } else if let Some(_) = args.subcommand_matches("help") {
        show_usage_examples(name);
        return Ok(());
    } else {
        return Err("invalid sub-command".to_string());
    }

    let server_uri = match args.value_of("server-uri") {
        Some(host) => host,
        None => return Err("need server URI".to_string()),
    };

    let writer = io::stdout();
    let logger = logging::create_logger(name, writer);

    let result = ttrpc::run_ttrpc(
        &logger,
        server_uri,
        server,
        interactive,
        commands,
        abstract_socket,
        use_vsock_crate_for_vsock,
    );
    if result.is_err() {
        eprintln!("error: {:?}", result.err());
        exit(1);
    }

    println!("result: no error: '{:?}'", result.ok());

    Ok(())
}

fn main() {
    match real_main() {
        Err(e) => {
            eprintln!("ERROR: {}", e);
            exit(1);
        }
        _ => (),
    };
}
