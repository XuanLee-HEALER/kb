//! Build libsimple (vendored at ../../vendor/libsimple) as a SQLite loadable
//! extension and stash the resulting `libsimple.{dylib,so}` plus jieba dict
//! files under `$OUT_DIR/libsimple-install/`.
//!
//! Two env vars are exposed to the crate at compile time:
//!   - `KB_LIBSIMPLE_DIR`  : absolute directory containing libsimple.{so,dylib}
//!   - `KB_LIBSIMPLE_DICT` : absolute directory containing *.utf8 jieba dicts
//!
//! The library is **not linked** into kb-server; SQLite loads it at runtime
//! via `load_extension()`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("repo root");
    let vendor_dir = repo_root.join("vendor").join("libsimple");

    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}/CMakeLists.txt",
        vendor_dir.display()
    );
    println!("cargo:rerun-if-changed={}/src", vendor_dir.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let build_dir = out_dir.join("libsimple-build");
    let install_dir = out_dir.join("libsimple-install");

    // Cheap up-to-date check: presence of the shared library marker file.
    let lib_name = shared_lib_name();
    let install_lib = install_dir.join("bin").join(&lib_name);
    let dict_dir = install_dir.join("bin").join("dict");
    let dict_marker = dict_dir.join("jieba.dict.utf8");

    if install_lib.exists() && dict_marker.exists() {
        emit_env(&install_dir);
        return;
    }

    fs::create_dir_all(&build_dir).expect("create libsimple build dir");

    // Configure.
    let mut cmake_args: Vec<String> = vec![
        format!("-S{}", vendor_dir.display()),
        format!("-B{}", build_dir.display()),
        "-DCMAKE_BUILD_TYPE=Release".into(),
        "-DBUILD_SQLITE3=OFF".into(),
        "-DBUILD_TEST_EXAMPLE=OFF".into(),
        "-DSIMPLE_WITH_JIEBA=ON".into(),
        format!("-DCMAKE_INSTALL_PREFIX={}", install_dir.display()),
    ];

    // Apple-specific: don't fat-build, target current arch only.
    if cfg!(target_os = "macos") {
        let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "arm64".into());
        let osx_arch = if arch == "aarch64" { "arm64" } else { &arch };
        cmake_args.push(format!("-DCMAKE_OSX_ARCHITECTURES={osx_arch}"));
    }

    run("cmake", &cmake_args);

    // Build + install.
    run(
        "cmake",
        &[
            "--build".into(),
            build_dir.display().to_string(),
            "--config".into(),
            "Release".into(),
            "-j".into(),
        ],
    );
    run(
        "cmake",
        &[
            "--install".into(),
            build_dir.display().to_string(),
            "--config".into(),
            "Release".into(),
        ],
    );

    assert!(
        install_lib.exists(),
        "libsimple build succeeded but {} not found",
        install_lib.display()
    );
    assert!(
        dict_marker.exists(),
        "libsimple build succeeded but jieba dict not installed at {}",
        dict_marker.display()
    );

    emit_env(&install_dir);
}

fn emit_env(install_dir: &Path) {
    let lib_dir = install_dir.join("bin");
    let dict_dir = lib_dir.join("dict");
    println!("cargo:rustc-env=KB_LIBSIMPLE_DIR={}", lib_dir.display());
    println!("cargo:rustc-env=KB_LIBSIMPLE_DICT={}", dict_dir.display());
}

fn shared_lib_name() -> String {
    if cfg!(target_os = "macos") {
        "libsimple.dylib".into()
    } else if cfg!(target_os = "windows") {
        "simple.dll".into()
    } else {
        "libsimple.so".into()
    }
}

fn run(prog: &str, args: &[String]) {
    let status = Command::new(prog)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn {prog}: {e}"));
    assert!(status.success(), "{prog} {args:?} failed: {status}");
}
