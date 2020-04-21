// Description: Server side of ttRPC comms

use crate::service::{HelloReply, HelloRequest, ShutdownReply, ShutdownRequest};
use crate::service_ttrpc::{create_my_service, MyService};
use crate::types::HelloService;
use nix::unistd::close;
use ttrpc::error::Error as TError;
use ttrpc::error::Result as TResult;
use ttrpc::server::Server;
use ttrpc::ttrpc::{Code, Status};
use ttrpc::TtrpcContext;

use slog::{error, info};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

pub type Result<T> = std::result::Result<T, String>;

impl<'a> MyService for HelloService {
    fn say_hello(&self, _ctx: &TtrpcContext, req: HelloRequest) -> TResult<HelloReply> {
        let msg = format!("Hello '{}'", req.get_name());

        info!(sl!(), "server responding";
            "client-request" => req.get_name(),
            "server-response" => msg.clone());

        let mut resp = HelloReply::default();

        resp.set_message(msg);

        Ok(HelloReply::new())
    }

    fn shutdown(&self, ctx: &TtrpcContext, req: ShutdownRequest) -> TResult<ShutdownReply> {
        info!(sl!(), "server responding";
            "command" => "shutdown",
            "client-request" => format!("{:?}", req));

        let cfg_ref = self.cfg.clone();
        let cfg = cfg_ref.lock().unwrap();

        if cfg.tx.is_none() {
            let err_msg = "No sender channel".to_string();

            error!(sl!(), "{}", err_msg);

            let mut status = Status::new();

            // FIXME:
            status.set_code(Code::NOT_FOUND);
            status.set_message("foo".to_string());

            return Err(TError::RpcStatus(status));
        }

        let tx = cfg.tx.as_ref().unwrap();

        info!(sl!(), "requesting shutdown");

        let result = tx.send(true);
        let _ = close(ctx.fd);

        info!(sl!(), "requested shutdown"; "result" => format!("{:?}", result));

        if result.is_err() {
            let mut status = Status::new();

            // FIXME:
            status.set_code(Code::NOT_FOUND);
            status.set_message(format!("{:?}", result.err()));

            return Err(TError::RpcStatus(status));
        }

        Ok(ShutdownReply::new())
    }
}

pub fn server(service: &HelloService, rx: Receiver<bool>) -> Result<()> {
    info!(sl!(), "starting");

    let s = Box::new(service.clone()) as Box<dyn MyService + Send + Sync>;
    let s = Arc::new(s);
    let the_service = create_my_service(s);

    let svc_ref = service.cfg.clone();
    let cfg = svc_ref.lock().unwrap();

    // Valid format schemes:
    //
    // - 'unix:///path/to/socket'
    //
    // - 'vsock://cid:port'
    //   (although CID is ignored as ttrpc hard-codes the cid
    //   as libc::VMADDR_CID_ANY, hence, "vsock://-1:port").
    let addr = &cfg.server_uri;

    let mut server = Server::new()
        .bind(&addr)
        .unwrap()
        .register_service(the_service);

    info!(sl!(), "setup complete"; "server-uri" => addr);

    // XXX: Critical - Allow the server handlers to access the
    // XXX: shared data.
    drop(cfg);

    server.start().expect("failed to start server");

    info!(sl!(), "started");

    info!(sl!(), "Waiting for server shutdown request");

    let _ = rx.recv();

    info!(sl!(), "Waited for server shutdown request");

    info!(sl!(), "Waiting for ttRPC server to end");
    server.shutdown();
    info!(sl!(), "Waited for ttRPC server to end");

    Ok(())
}
