//! Script to build protobuf defined types and tonic based gRPC service stubs.
//! This is intentionally not part of the workspace in order to avoid blocking 
//! code generation on the rest of the workspace compiling.

use std::path::PathBuf;
use std::path::Path;

fn main() {
    let crate_root = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root.parent().unwrap();

    // Example of how to do codegen (we can remove this once we actually have code to generate)
    codegen(
        &repo_root.join("apps").join("reshard").join("host"),
        &["proto/reshard.proto"],
        &["proto"],
        true,
        true
    )
}

fn codegen(
    root_dir: &Path,
    proto_files: &[&str],
    include_dirs: &[&str],
    build_server: bool,
    build_client: bool
) {
    let out_dir = root_dir.join("src").join("generated");
    let proto_files: Vec<_> = proto_files.into_iter().map(|path| root_dir.join(path)).collect();
    let include_dirs: Vec<_> = include_dirs.into_iter().map(|path| root_dir.join(path)).collect();

    tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("descriptor.bin"))
        .out_dir(out_dir)
        .build_server(build_server)
        .build_client(build_client)
        .compile_protos(
            &proto_files,
            &include_dirs,
        ).unwrap();
}
