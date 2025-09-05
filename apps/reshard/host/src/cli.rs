use crate::run;
use crate::ReshardHostConfig;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use qos_core::io::SocketAddress;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    cid: Option<u32>,

    #[arg(long)]
    port: Option<u32>,

    #[arg(long)]
    usock: Option<String>,

    #[arg(long)]
    host_ip: String,

    #[arg(long)]
    host_port: u16,

    #[arg(long)]
    vsock_to_host: bool,
}

impl Args {
    /// Address the host server should listen on.
    fn host_addr(&self) -> SocketAddr {
        let ip = Ipv4Addr::from_str(&self.host_ip).expect("could not parse ip to IP v4");
        SocketAddr::new(IpAddr::V4(ip), self.host_port)
    }

    /// Get the `SocketAddress` for the enclave server.
    ///
    /// # Panics
    ///
    /// Panics if the options are not valid for exactly one of unix or vsock.
    fn enclave_addr(&self) -> SocketAddress {
        match (self.cid, self.port, &self.usock) {
            #[cfg(feature = "vsock")]
            (Some(c), Some(p), None) => SocketAddress::new_vsock(c, p, self.vsock_to_host_flag()),
            (None, None, Some(u)) => SocketAddress::new_unix(u),
            _ => panic!("Invalid socket options"),
        }
    }

    #[cfg(feature = "vsock")]
    fn vsock_to_host_flag(&self) -> u8 {
        if self.vsock_to_host {
            println!("Configuring vsock with VMADDR_FLAG_TO_HOST.");
            qos_core::io::VMADDR_FLAG_TO_HOST
        } else {
            println!("Configuring vsock with VMADDR_NO_FLAGS.");
            qos_core::io::VMADDR_NO_FLAGS
        }
    }
}

/// Host server command line interface.
pub struct CLI;
impl CLI {
    /// Execute the command line interface.
    pub async fn execute() {
        let args = Args::parse();

        run(ReshardHostConfig {
            listen_addr: args.host_addr(),
            enclave_addr: args.enclave_addr(),
        })
        .await
        .unwrap();
    }
}
