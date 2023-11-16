// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    env,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> Result<()> {
    #[cfg(not(target_env = "msvc"))]
    std::env::set_var("PROTOC", protobuf_src::protoc());

    let out_dir = if env::var("DUMP_GENERATED_GRPC").is_ok() {
        PathBuf::from("")
    } else {
        PathBuf::from(env::var("OUT_DIR")?)
    };

    let dirs = &["proto"];

    //Scalar note: Build tofnd.proto with default config
    tonic_build::configure()
        .out_dir(&out_dir)
        .compile(&["proto/tofnd.proto"], dirs)?;

    // Use `Bytes` instead of `Vec<u8>` for bytes fields
    let mut config = prost_build::Config::new();
    config.bytes(["."]);

    build_tss_service(&out_dir);
    build_scalar_event_service(&out_dir);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-env-changed=DUMP_GENERATED_GRPC");

    nightly();
    beta();
    stable();

    Ok(())
}

fn build_tss_service(out_dir: &Path) {
    let tss_service = anemo_build::manual::Service::builder()
        .name("TssPeer")
        .package("tss.network")
        .method(
            anemo_build::manual::Method::builder()
                .name("keygen")
                .route_name("KeyGen")
                .request_type("crate::TssAnemoKeygenRequest")
                .response_type("crate::TssAnemoKeygenResponse")
                .codec_path("anemo::rpc::codec::BincodeCodec")
                // .codec_path("anemo::rpc::codec::JsonCodec")
                // .server_handler_return_raw_bytes(true)
                .build(),
        )
        .method(
            anemo_build::manual::Method::builder()
                .name("sign")
                .route_name("Sign")
                .request_type("crate::TssAnemoSignRequest")
                .response_type("crate::TssAnemoSignResponse")
                .codec_path("anemo::rpc::codec::BincodeCodec")
                // .codec_path("anemo::rpc::codec::JsonCodec")
                // .server_handler_return_raw_bytes(true)
                .build(),
        )
        .method(
            anemo_build::manual::Method::builder()
                .name("abort")
                .route_name("Abort")
                .request_type("crate::AbortRequest")
                .response_type("crate::AbortResponse")
                .codec_path("anemo::rpc::codec::BincodeCodec")
                .build(),
        )
        // .method(
        //     anemo_build::manual::Method::builder()
        //         .name("key_presence")
        //         .route_name("KeyPresence")
        //         .request_type("crate::KeyPresenceRequest")
        //         .response_type("crate::KeyPresenceResponse")
        //         .codec_path("anemo::rpc::codec::BincodeCodec")
        //         .build(),
        // )
        .build();
    anemo_build::manual::Builder::new()
        .out_dir(out_dir)
        .compile(&[tss_service]);
}

fn build_scalar_event_service(out_dir: &Path) {
    let codec_path = "mysten_network::codec::anemo::BcsSnappyCodec";

    let scalar_event_service = anemo_build::manual::Service::builder()
        .name("ScalarEvent")
        .package("scalar")
        .method(
            anemo_build::manual::Method::builder()
                .name("request_event_verify")
                .route_name("RequestEventVerify")
                .request_type("crate::RequestVerifyRequest")
                .response_type("crate::RequestVerifyResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            anemo_build::manual::Method::builder()
                .name("create_cross_chain_transaction")
                .route_name("CreateCrossChainTransaction")
                .request_type("crate::CrossChainTransaction")
                .response_type("()")
                .codec_path(codec_path)
                .build(),
        )
        .build();
    anemo_build::manual::Builder::new()
        .out_dir(out_dir)
        .compile(&[scalar_event_service]);
}

#[rustversion::nightly]
fn nightly() {
    println!("cargo:rustc-cfg=nightly");
}

#[rustversion::not(nightly)]
fn nightly() {}

#[rustversion::beta]
fn beta() {
    println!("cargo:rustc-cfg=beta");
}

#[rustversion::not(beta)]
fn beta() {}

#[rustversion::stable]
fn stable() {
    println!("cargo:rustc-cfg=stable");
}

#[rustversion::not(stable)]
fn stable() {}
