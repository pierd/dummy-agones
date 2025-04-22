fn main() {
    let derive_serde =
        "#[derive(serde::Deserialize, serde::Serialize)] #[serde(rename_all = \"camelCase\")]";
    tonic_build::configure()
        .build_server(true)
        .type_attribute("GameServer", derive_serde)
        .type_attribute("ObjectMeta", derive_serde)
        .type_attribute("Spec", derive_serde)
        .type_attribute("Health", derive_serde)
        .type_attribute("Status", derive_serde)
        .type_attribute("PlayerStatus", derive_serde)
        .type_attribute("Port", derive_serde)
        .type_attribute("Address", derive_serde)
        .type_attribute("ListStatus", derive_serde)
        .type_attribute("CounterStatus", derive_serde)
        .compile(
            &["proto/sdk/alpha/alpha.proto", "proto/sdk/sdk.proto"],
            &[
                "proto/googleapis",
                "proto/grpc-gateway",
                "proto/sdk/alpha",
                "proto/sdk",
            ],
        )
        .expect("failed to compile protobuffers");
}
