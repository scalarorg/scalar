fn main() {
    println!("cargo:rerun-if-changed=proto");
    build_consensus_service();
}

fn build_consensus_service() {
    tonic_build::configure()
        .out_dir("src/proto")
        .compile(&["proto/consensus.proto"], &["proto"])
        .expect("Failed to compile proto(s)");
}
