use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
fn collect(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect(&path, paths);
        } else if path.extension().is_some_and(|e| e == "rs") {
            paths.push(path);
        }
    }
}
fn main() {
    let mut paths = Vec::new();
    for dir in ["src", "../graph-core/src"] {
        println!("cargo:rerun-if-changed={dir}");
        collect(Path::new(dir), &mut paths);
    }
    for file in [
        "build.rs",
        "Cargo.toml",
        "../graph-core/Cargo.toml",
        "../../Cargo.toml",
        "../../Cargo.lock",
        "../../rust-toolchain.toml",
    ] {
        paths.push(file.into());
    }
    paths.sort();
    let mut hash = Sha256::new();
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.display());
        hash.update(path.to_str().expect("UTF-8 source path"));
        hash.update([0]);
        hash.update(std::fs::read(path).expect("source bytes"));
        hash.update([0]);
    }
    println!("cargo:rustc-env=ENGINE_SOURCE_SHA256={:x}", hash.finalize());
}
