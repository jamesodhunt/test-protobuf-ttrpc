// Description: Client side of ttRPC comms

use crate::service::{HelloRequest, ShutdownRequest};
use crate::service_ttrpc::MyServiceClient;
use crate::types::{Config, HelloService};
use nix::sys::socket::{
    connect, socket, AddressFamily, SockAddr, SockFlag, SockType, UnixAddr, VsockAddr,
};
use slog::info;
use std::io;
use std::io::Write;
use std::os::unix::io::{IntoRawFd, RawFd};
use std::os::unix::net::UnixStream;
use ttrpc::client::Client;
use vsock::VsockStream;

pub type Result<T> = std::result::Result<T, String>;

type FP = fn(cfg: &Config, client: &MyServiceClient, data: &str) -> Result<()>;

const TIMEOUT_NANO: i64 = 0;

struct Cmd {
    name: &'static str,
    fp: FP,
}

const SHUTDOWN_CMD: &str = "Shutdown";

static CMDS: &[Cmd] = &[
    Cmd {
        name: "SayHello",
        fp: cmd_say_hello,
    },
    Cmd {
        name: SHUTDOWN_CMD,
        fp: cmd_shutdown,
    },
];

fn get_cmd_names() -> Vec<String> {
    let mut names = Vec::new();

    for cmd in CMDS {
        names.push(cmd.name.to_string());
    }

    names
}

fn get_cmd_func(name: &str) -> Result<FP> {
    for cmd in CMDS {
        if cmd.name == name {
            return Ok(cmd.fp);
        }
    }

    Err(format!("Invalid command: {:?}", name))
}

fn client_create_vsock_fd_with_vsock_crate(cid: libc::c_uint, port: u32) -> Result<RawFd> {
    info!(sl!(), "XXX: using VSOCK crate for client VSOCK comms");

    let vsock_addr = VsockAddr::new(cid, port);
    let sock_addr = SockAddr::Vsock(vsock_addr);

    let stream = match VsockStream::connect(&sock_addr) {
        Ok(s) => s,
        Err(e) => return Err(e.to_string()),
    };

    let fd = stream.into_raw_fd();

    Ok(fd)
}

fn client_create_vsock_fd_with_nix_crate(cid: libc::c_uint, port: u32) -> Result<RawFd> {
    info!(sl!(), "XXX: using NIX crate for client VSOCK comms");

    let fd = match socket(
        AddressFamily::Vsock,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        None,
    ) {
        Ok(fd) => fd,
        Err(e) => return Err(e.to_string()),
    };

    let sock_addr = SockAddr::new_vsock(cid, port);

    match connect(fd, &sock_addr) {
        Ok(_) => (),
        Err(e) => return Err(e.to_string()),
    };

    Ok(fd)
}

// Create a vsock socket using either the vsock crate, or the nix crate.
fn client_create_vsock_fd(
    use_vsock_crate_for_vsock: bool,
    cid: libc::c_uint,
    port: u32,
) -> Result<RawFd> {
    match use_vsock_crate_for_vsock {
        true => client_create_vsock_fd_with_vsock_crate(cid, port),
        false => client_create_vsock_fd_with_nix_crate(cid, port),
    }
}

fn client_create_fd(
    server_uri: &str,
    force_abstract_socket: bool,
    use_vsock_crate_for_vsock: bool,
) -> Result<RawFd> {
    // Cribbed from ttrpc:server.rs
    let hostv: Vec<&str> = server_uri.trim().split("://").collect();

    if hostv.len() != 2 {
        return Err(format!("Invalid URI: {:?}", server_uri));
    }

    let scheme = hostv[0].to_lowercase();

    let fd: RawFd;

    match scheme.as_str() {
        "unix" => {
            let mut abstract_socket = force_abstract_socket;

            let mut path = hostv[1].to_string();

            if path.starts_with('@') || abstract_socket {
                abstract_socket = true;

                // XXX: ESSENTIAL! Add a trailing terminator
                // XXX: as this is what the "ttrpc" crate does!
                path = path + &"\x00".to_string();
            }

            if abstract_socket {
                fd = match socket(
                    AddressFamily::Unix,
                    SockType::Stream,
                    SockFlag::empty(),
                    None,
                ) {
                    Ok(s) => s,
                    Err(e) => return Err(format!("Failed to create Unix Domain socket: {:?}", e)),
                };

                let mut unix_addr = match UnixAddr::new_abstract(path.as_bytes()) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(format!(
                            "Failed to create Unix Domain abstract socket: {:?}",
                            e
                        ))
                    }
                };

                // FIXME: Fix BUG: https://github.com/nix-rust/nix/pull/1120/
                //
                // All versions of the nix package prior to 0.16.0 contained a bug
                // where the length of the path specified to new_abstract() was not
                // calculated - nix assumed the maximum path length. This meant the
                // path contained trailing nulls which is perfectly valid for an
                // abstract socket... it just means you cannot connect to it from
                // well-behaved programs that correctly handle the path length!!
                //
                // The problem for this program is that it currently relies on the
                // vsock crate, which depends on "nix = 0.15.1".
                unix_addr.1 = path.len() + 1;

                let sock_addr = SockAddr::Unix(unix_addr);

                match connect(fd, &sock_addr) {
                    Ok(_) => (),
                    Err(e) => {
                        return Err(format!(
                            "Failed to connect to Unix Domain abstract socket: {:?}",
                            e
                        ))
                    }
                };
            } else {
                let stream = match UnixStream::connect(path) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(format!(
                            "failed to create named UNIX Domain stream socket: {:?}",
                            e
                        ))
                    }
                };

                fd = stream.into_raw_fd();
            }
        }
        "vsock" => {
            let addr: Vec<&str> = hostv[1].split(':').collect();
            if addr.len() != 2 {
                return Err(format!("Invalid VSOCK URI: {:?}", server_uri));
            }

            let cid: u32 = match addr[0] {
                "-1" => libc::VMADDR_CID_ANY,
                "" => libc::VMADDR_CID_ANY,
                _ => match addr[0].parse::<u32>() {
                    Ok(c) => c,
                    Err(e) => return Err(format!("VSOCK cid is not numeric: {:?}", e)),
                },
            };

            let port: u32 = match addr[1].parse::<u32>() {
                Ok(r) => r,
                Err(e) => return Err(format!("VSOCK port is not numeric: {:?}", e)),
            };

            fd = client_create_vsock_fd(use_vsock_crate_for_vsock, cid, port)?;
        }
        _ => return Err(format!("invalid address scheme: {:?}", server_uri)),
    };

    Ok(fd)
}

pub fn client(service: &HelloService, commands: Vec<&str>) -> Result<()> {
    info!(sl!(), "starting");

    let svc_ref = service.cfg.clone();
    let cfg = svc_ref.lock().unwrap();

    let addr = &cfg.server_uri;

    let fd = match client_create_fd(
        addr,
        cfg.force_abstract_socket,
        cfg.use_vsock_crate_for_vsock,
    ) {
        Ok(fd) => fd,
        Err(e) => return Err(format!("failed to create client fd: {:?}", e)),
    };

    let ttrpc_client = Client::new(fd);

    let client = MyServiceClient::new(ttrpc_client);

    info!(sl!(), "setup complete";
        "server-address" => addr);

    if cfg.interactive {
        return interactive_client_loop(&cfg, &client);
    }

    for cmd in commands {
        let (result, shutdown) = handle_cmd(&cfg, &client, &cmd);
        if result.is_err() {
            return result;
        }

        if shutdown {
            break;
        }
    }

    Ok(())
}

// Execute the ttRPC specified by the first field of "line". Return a result
// along with a bool which if set means the client should shutdown.
fn handle_cmd(cfg: &Config, client: &MyServiceClient, line: &str) -> (Result<()>, bool) {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let name = fields[0];

    let f = match get_cmd_func(&name) {
        Ok(fp) => fp,
        Err(e) => return (Err(e), false),
    };

    let args = if fields.len() > 1 {
        fields[1..].join(" ")
    } else {
        String::new()
    };

    let result = f(cfg, client, &args);
    if result.is_err() {
        return (result, false);
    }

    info!(sl!(), "Command {:} returned {:?}", name, result);

    let shutdown = name == SHUTDOWN_CMD;

    (Ok(()), shutdown)
}

fn interactive_client_loop(cfg: &Config, client: &MyServiceClient) -> Result<()> {
    let names = get_cmd_names();
    let quit = "quit";

    loop {
        println!("Commands ('{}' to end):\n", quit);

        names.iter().for_each(|n| println!(" - {}", n));

        println!();

        let line = readline("Enter command").expect("failed to read line");

        if line == "" || line == "\n" {
            continue;
        }

        if line.starts_with(quit) {
            break;
        }

        let (result, shutdown) = handle_cmd(cfg, client, &line);
        if result.is_err() {
            return result;
        }

        if shutdown {
            break;
        }
    }

    Ok(())
}

fn readline(prompt: &str) -> std::result::Result<String, String> {
    print!("{}: ", prompt);

    match io::stdout().flush() {
        Ok(_) => (),
        _ => return Err("failed to flush".to_string()),
    };

    let mut line = String::new();

    match std::io::stdin().read_line(&mut line) {
        // Remove NL
        Ok(_) => Ok(line.trim_end().to_string()),

        _ => Err("failed to read line".to_string()),
    }
}

fn cmd_say_hello(_cfg: &Config, client: &MyServiceClient, msg: &str) -> Result<()> {
    let mut req = HelloRequest::default();

    req.set_name(msg.to_owned());

    info!(sl!(), "sending request to server";
        "request" => msg);

    let reply = client.say_hello(&req, TIMEOUT_NANO).expect("rpc");
    info!(sl!(), "response received";
        "response" => reply.get_message());

    Ok(())
}

fn cmd_shutdown(_cfg: &Config, client: &MyServiceClient, _msg: &str) -> Result<()> {
    info!(sl!(), "cmd_shutdown: requesting shutdown");

    let req = ShutdownRequest::default();

    let reply = client.shutdown(&req, TIMEOUT_NANO).expect("rpc");

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}
