mod client;
mod server;

mod wire {
    tonic::include_proto!("x05.probe.v1");
}

use std::{env, error::Error, future::pending};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn Error>> {
    match env::args().nth(1).as_deref() {
        Some("server") => server::serve().await,
        Some("client") => client::run().await,
        Some("idle") => pending().await,
        _ => Err("usage: ieo-x05-probe server|client|idle".into()),
    }
}
