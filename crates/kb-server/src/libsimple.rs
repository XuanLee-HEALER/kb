//! Locate libsimple.{so,dylib} + jieba dict directory at runtime.
//!
//! Resolution order:
//!   1. `KB_LIBSIMPLE_DIR` / `KB_LIBSIMPLE_DICT` env vars (runtime override).
//!   2. `<exe_dir>/libsimple/` (next to the binary, for deployed builds).
//!   3. Compile-time embedded paths from build.rs (dev / `cargo run`).

use std::path::PathBuf;

const BUILD_LIB_DIR: &str = env!("KB_LIBSIMPLE_DIR");
const BUILD_DICT_DIR: &str = env!("KB_LIBSIMPLE_DICT");

#[must_use]
pub fn lib_dir() -> PathBuf {
    if let Ok(p) = std::env::var("KB_LIBSIMPLE_DIR") {
        return PathBuf::from(p);
    }
    if let Some(p) = exe_relative("libsimple") {
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(BUILD_LIB_DIR)
}

#[must_use]
pub fn dict_dir() -> PathBuf {
    if let Ok(p) = std::env::var("KB_LIBSIMPLE_DICT") {
        return PathBuf::from(p);
    }
    if let Some(p) = exe_relative("libsimple/dict") {
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(BUILD_DICT_DIR)
}

#[must_use]
pub fn lib_filename() -> &'static str {
    if cfg!(target_os = "macos") {
        "libsimple.dylib"
    } else if cfg!(target_os = "windows") {
        "simple.dll"
    } else {
        "libsimple.so"
    }
}

#[must_use]
pub fn lib_path() -> PathBuf {
    lib_dir().join(lib_filename())
}

fn exe_relative(rel: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(rel))
}
