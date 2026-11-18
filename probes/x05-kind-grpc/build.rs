fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=probe.proto");
    let descriptors = protox::compile(["probe.proto"], ["."])?;
    tonic_prost_build::configure().compile_fds(descriptors)?;
    Ok(())
}
