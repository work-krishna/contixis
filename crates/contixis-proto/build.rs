fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the vendored protoc binary so we don't need a system install.
    let protoc = protoc_bin_vendored::protoc_bin_path().unwrap();
    std::env::set_var("PROTOC", protoc);

    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .field_attribute(".", "#[serde(default)]")
        .compile_protos(&["proto/contixis.proto"], &["proto/"])?;
    Ok(())
}
