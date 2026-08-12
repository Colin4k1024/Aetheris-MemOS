fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false) // no client needed — SDKs use REST
        .out_dir("src/protocol/gen")
        .compile_protos(&["proto/memory.proto"], &["proto/"])?;
    Ok(())
}
