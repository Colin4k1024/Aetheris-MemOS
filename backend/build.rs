fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only regenerate if protoc is available. The generated code is committed to
    // `src/protocol/gen/` so CI and contributors without protoc installed can
    // build from the checked-in output. If you modify `proto/memory.proto`, run
    // `cargo build` locally (with protoc in PATH) and commit the regenerated file.
    if std::process::Command::new("protoc")
        .arg("--version")
        .output()
        .is_ok()
    {
        tonic_build::configure()
            .build_server(true)
            .build_client(false)
            .out_dir("src/protocol/gen")
            .compile_protos(&["proto/memory.proto"], &["proto/"])?;
    } else {
        println!(
            "cargo:warning=protoc not found — using committed generated code in src/protocol/gen/"
        );
    }
    Ok(())
}
