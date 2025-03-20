use std::io::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .out_dir("src/protobuf")
        .type_attribute(".", "#[derive(serde::Serialize)]")
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &[
                "./proto/shared.proto",
                "./proto/sdk/session.proto",
                "./proto/sdk/features.proto",
                "./proto/sdk/features.mixer.proto",
                "./proto/sdk/gateway.proto",
                "./proto/cluster/media.proto",
                "./proto/cluster/gateway.proto",
                "./proto/cluster/connector.proto",
            ],
            &["./proto"],
        )?;
    Ok(())
}
