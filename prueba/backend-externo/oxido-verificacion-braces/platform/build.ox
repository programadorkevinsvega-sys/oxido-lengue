sintaxis llaves
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let compiler_path = protoc_bin_vendored::protoc_bin_path()?;
    let mut prost_configuration = tonic_prost_build::Config::new();
    prost_configuration.protoc_executable(compiler_path);
    tonic_prost_build::configure().compile_with_config(
        prost_configuration,
        &["proto/xovyra/runtime/v1/runtime.proto"],
        &["proto"],
    )?;
    println!("cargo:rerun-if-changed=proto/xovyra/runtime/v1/runtime.proto");
    Ok(())
}
