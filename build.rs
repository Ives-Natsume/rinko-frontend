fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false) // Frontend doesn't need server code
        .build_client(true)
        .compile_protos(&["proto/bot.proto"], &["proto"])?;
    
    println!("cargo:rerun-if-changed=proto/bot.proto");
    Ok(())
}
