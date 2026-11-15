//! Generates ieo.internal.v0 from contracts/proto at build time. protox parses the .proto files
//! in Rust, so neither CI nor a developer needs protoc.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = "../../contracts/proto";
    let file = "ieo/internal/v0/supervisor.proto";
    println!("cargo:rerun-if-changed={root}/{file}");
    let fds = protox::compile([file], [root])?;
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_fds(fds)?;
    Ok(())
}
