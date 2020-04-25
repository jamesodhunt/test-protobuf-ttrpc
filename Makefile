#---------------------------------------------------------------------
# Description: Noddy rust ttrpc example
# Date: 2020-03-25
# Author: James Hunt <jamesodhunt@gmail.com>
#
# XXX: See further documentation on: https://crates.io/crates/ttrpc
#---------------------------------------------------------------------

CC = protoc

# Run:
#
# $ git clone https://github.com/containerd/ttrpc-rust
# $ cd ttrpc-rust/compiler
# $ cargo install --force --path .
#
TTRPC_RUST_PLUGIN = $(HOME)/.cargo/bin/ttrpc_rust_plugin

# The name of the ttrpc service
SERVICE = service

# Directory containing service definitions
SERVICE_DIR = $(PWD)/$(SERVICE)

# Where to put the generated code
GENERATED_DIR = $(PWD)/src

# Protocol buffer definition of the service
PROTOCOL_FILENAME = $(SERVICE).proto

PROTOCOL_FILE = $(SERVICE_DIR)/$(PROTOCOL_FILENAME)

GENERATED_FILES =

GENERATED_FILES += $(GENERATED_DIR)/$(SERVICE).rs
GENERATED_FILES += $(GENERATED_DIR)/$(SERVICE)_ttrpc.rs

#---------------------------------------------------------------------
# Program arguments

UNIX_SERVER_URI ?= "unix:///tmp/my.socket"

VSOCK_PORT ?= 1024

# -1 means "libc::VMADDR_CID_ANY"
VSOCK_SERVER_CID ?= -1

# Assumes:
#
# "qemu-system-x86_64 -device vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid=3"
VSOCK_CLIENT_CID ?= 3

VSOCK_SERVER_URI ?= "vsock://$(VSOCK_SERVER_CID):$(VSOCK_PORT)"

VSOCK_CLIENT_URI ?= "vsock://$(VSOCK_CLIENT_CID):$(VSOCK_PORT)"

#---------------------------------------------------------------------

default: server

# XXX:
#
# --rust_out=: generate the "service.rs" file
#   (which contains the request and reply types).
# --ttrpc_out=: generate the "service_ttrpc.rs" file
#   (which contains the public trait for the ttrpc service and its functions).
generate-service:
	@$(CC) --version
	(cd $(SERVICE_DIR) && \
		$(CC) \
		-I$(SERVICE_DIR) \
		--plugin=protoc-gen-ttrpc=$(TTRPC_RUST_PLUGIN) \
		--rust_out=$(GENERATED_DIR) \
		--ttrpc_out=$(GENERATED_DIR) \
		$(PROTOCOL_FILENAME))

build-driver:
	cargo build -v

build: generate-service build-driver

client: build unix-client
server: build unix-server

CLIENT_COMMANDS = \
    --commands "SayHello world" \
    --commands "SayHello world" \
    --commands "Shutdown"

unix-server:
	cargo run -v -- --server-uri $(UNIX_SERVER_URI) server

vsock-server:
	cargo run -v -- --server-uri $(VSOCK_SERVER_URI) server

unix-client:
	cargo run -v -- --server-uri $(UNIX_SERVER_URI) --abstract client $(CLIENT_COMMANDS)

vsock-client:
	cargo run -v -- --server-uri $(VSOCK_CLIENT_URI) --abstract client --crate-for-vsock=nix $(CLIENT_COMMANDS)

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

clean:
	cargo clean
	rm -f $(GENERATED_FILES)
