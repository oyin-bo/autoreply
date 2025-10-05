fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "experimental-sentencepiece")]
    {
        prost_build::Config::new()
            .protoc_arg("--experimental_allow_proto3_optional")
            .compile_protos(&["proto/sentencepiece_model.proto"], &["proto/"])?;
    }
    Ok(())
}
