use glob::glob;
use std::env;
use std::{path::PathBuf, process::Command};

/// Get the path to the repo root (uses git to find the repo root)
fn get_repo_root() -> PathBuf {
    let repo_root = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .unwrap()
        .stdout;

    let repo_root = String::from_utf8(repo_root).unwrap();
    let repo_root = repo_root.trim();
    let repo_root = PathBuf::from(repo_root);

    repo_root
}

/// Get the path to WASI-SDK root (uses git to find the repo root)
fn get_wasi_sdk_root() -> PathBuf {
    let repo_root = get_repo_root();
    repo_root.join("deps/wasi-sdk")
}

/// Get the path to the WASI-SDK cc binary
fn get_wasi_cc() -> PathBuf {
    get_wasi_sdk_root().join("bin/clang")
}

/// Get the path to the WASI-SDK ar binary
fn get_wasi_ar() -> PathBuf {
    get_wasi_sdk_root().join("bin/llvm-ar")
}

fn main() {
    println!("cargo:rerun-if-changed=src/lfs");
    println!("cargo:rerun-if-changed=src/lfs-sys");
    println!("cargo:rerun-if-changed=src/lfs-rambd");

    let files = glob("*.o").unwrap().chain(glob("*.a").unwrap());
    files.for_each(|f| {
        let f = f.unwrap();
        Command::new("rm").arg("-f").arg(f).output().unwrap();
    });

    let clang_ar = get_wasi_ar();
    let clang_cc = get_wasi_cc();

    let out = Command::new(clang_cc)
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-Wpedantic")
        .arg("-O3")
        .arg("-static")
        .arg("-fPIC")
        .arg("-DLFS_NO_DEBUG")
        .arg("-DLFS_NO_WARN")
        .arg("-DLFS_NO_ERROR")
        .arg("-DLFS_NO_ASSERT")
        .arg("-DLFS_THREADSAFE")
        .arg("-mmutable-globals")
        .arg("-matomics")
        .arg("-mbulk-memory")
        .arg("-c")
        .arg("-Isrc/lfs")
        .arg("-Isrc/lfs-sys")
        .arg("-Isrc/lfs-rambd")
        .arg("src/lfs/lfs.c")
        .arg("src/lfs/lfs_util.c")
        .arg("src/lfs-sys/lfs_sys.c")
        .arg("src/lfs-rambd/lfs_rambd.c")
        .output()
        .unwrap();

    if !out.status.success() {
        println!("stdout: {}", String::from_utf8_lossy(&out.stdout));
        println!("stderr: {}", String::from_utf8_lossy(&out.stderr));
        panic!("failed to compile lfs");
    }

    let out = Command::new(clang_ar)
        .arg("-r")
        .arg("liblfs.a")
        .arg("lfs.o")
        .arg("lfs_sys.o")
        .arg("lfs_util.o")
        .arg("lfs_rambd.o")
        .output()
        .unwrap();

    if !out.status.success() {
        println!("stdout: {}", String::from_utf8_lossy(&out.stdout));
        println!("stderr: {}", String::from_utf8_lossy(&out.stderr));
        panic!("failed to compile lfs");
    }

    let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}", dir);
    println!("cargo:rustc-link-lib=lfs");
}
