fn build_proto() {
    tonic_build::compile_protos("proto/arcanist.proto")
        .unwrap_or_else(|e| panic!("failed to compile proto: {}", e));
    println!("cargo:rerun-if-changed=proto/arcanist.proto");
}

fn main() {
    build_proto();
    println!("cargo:rerun-if-changed=build.rs");
}
