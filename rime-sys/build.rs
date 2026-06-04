// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: MIT

use std::env;
use std::path::PathBuf;

fn main() {
    let mut include_dirs: Vec<String> = Vec::new();
    let mut is_static = false;

    // Try pkg-config first
    if pkg_config::probe_library("rime").is_ok() {
        if let Ok(lib) = pkg_config::Config::new().probe("rime") {
            include_dirs = lib.include_paths.iter().map(|p| p.to_string_lossy().into()).collect();
            // Check if linking against a static library
            is_static = lib.link_paths.iter().any(|dir| {
                dir.join("librime.a").exists() || dir.join("librime-static.a").exists()
            });
        }
    }

    // Fallback to environment variables or defaults
    if include_dirs.is_empty() {
        let include_dir =
            env::var("RIME_INCLUDE_DIR").unwrap_or_else(|_| "/usr/include".to_owned());
        let lib_dir = env::var("RIME_LIB_DIR").unwrap_or_else(|_| "/usr/lib".to_owned());

        println!("cargo:rustc-link-search={lib_dir}");
        println!("cargo:rustc-link-lib=rime");

        is_static = PathBuf::from(&lib_dir).join("librime.a").exists();

        include_dirs.push(include_dir);
    }

    // Static librime needs C++ standard library.
    // Transitive dependencies are declared in rime.pc (Libs/Libs.private).
    if is_static {
        if cfg!(target_os = "macos") {
            println!("cargo:rustc-link-lib=c++");
        } else {
            println!("cargo:rustc-link-lib=stdc++");
        }
    }

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for dir in &include_dirs {
        builder = builder.clang_arg(format!("-I{dir}"));
    }

    // Add local include/ for keycodes.h and modifiers.h
    builder = builder.clang_arg("-Iinclude");

    let bindings = builder
        .generate()
        .expect("Unable to generate bindings for librime");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
