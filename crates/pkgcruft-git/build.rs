fn build_proto() {
    tonic_prost_build::configure()
        .compile_protos(&["proto/pkgcruft.proto"], &["proto"])
        .unwrap_or_else(|e| panic!("failed to compile proto: {e}"));
    println!("cargo::rerun-if-changed=proto/pkgcruft.proto");
}

fn main() {
    build_proto();
    println!("cargo::rerun-if-changed=build.rs");
}
