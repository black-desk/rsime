// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: MIT

use std::env;
use std::path::PathBuf;

#[cfg(feature = "bundled-vcpkg")]
use std::path::Path;

#[cfg(feature = "bundled-vcpkg")]
fn find_vcpkg_root() -> Result<PathBuf, String> {
    // 1. Check VCPKG_ROOT environment variable
    if let Ok(root) = env::var("VCPKG_ROOT") {
        let path = PathBuf::from(&root);
        if path.join("vcpkg").exists() {
            return Ok(path);
        }
    }
    // 2. Check PATH for vcpkg executable
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let vcpkg_bin = dir.join("vcpkg");
            if vcpkg_bin.exists() {
                if let Some(root) = vcpkg_bin.parent() {
                    return Ok(root.to_path_buf());
                }
            }
        }
    }
    Err(
        "bundled-vcpkg feature requires vcpkg. Set VCPKG_ROOT or install vcpkg in PATH.".to_string(),
    )
}

#[cfg(feature = "bundled-vcpkg")]
fn vcpkg_triplet() -> String {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    match (arch.as_str(), os.as_str()) {
        ("x86_64", "linux") => "x64-linux".to_string(),
        ("aarch64", "linux") => "arm64-linux".to_string(),
        ("x86_64", "macos") => "x64-osx".to_string(),
        ("aarch64", "macos") => "arm64-osx".to_string(),
        _ => format!("unsupported triplet: arch={arch}, os={os}"),
    }
}

#[cfg(feature = "bundled-vcpkg")]
fn run_vcpkg_install(manifest_dir: &Path, vcpkg_root: &Path) -> Result<PathBuf, String> {
    let vcpkg_bin = vcpkg_root.join("vcpkg");

    let triplet = vcpkg_triplet();

    let status = std::process::Command::new(&vcpkg_bin)
        .args(["install", "--triplet", &triplet, "--allow-unsupported"])
        .current_dir(manifest_dir)
        .env("VCPKG_ROOT", vcpkg_root)
        .status()
        .map_err(|e| format!("Failed to execute vcpkg: {e}"))?;

    if !status.success() {
        return Err(format!("vcpkg install failed with status: {status}"));
    }

    let installed_dir = manifest_dir.join(format!("vcpkg_installed/{triplet}"));
    if !installed_dir.exists() {
        return Err(format!(
            "vcpkg installed directory not found: {}",
            installed_dir.display()
        ));
    }

    Ok(installed_dir)
}

fn main() {
    // === bundled-vcpkg feature: install librime via vcpkg ===
    #[cfg(feature = "bundled-vcpkg")]
    {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let vcpkg_root =
            find_vcpkg_root().expect("bundled-vcpkg feature requires vcpkg to be installed");
        let installed_dir = run_vcpkg_install(&manifest_dir, &vcpkg_root)
            .expect("vcpkg install for librime failed");

        let pkg_config_dir = installed_dir.join("lib").join("pkgconfig");
        if pkg_config_dir.exists() {
            let existing = env::var("PKG_CONFIG_PATH").unwrap_or_default();
            let new_path = if existing.is_empty() {
                pkg_config_dir.to_string_lossy().into_owned()
            } else {
                format!("{}:{}", pkg_config_dir.to_string_lossy(), existing)
            };
            // SAFETY: build.rs is single-threaded; setting an env var here is safe.
            unsafe { env::set_var("PKG_CONFIG_PATH", &new_path); }
        }
    }

    let lib = pkg_config::Config::new()
        .probe("rime")
        .expect("Failed to find librime via pkg-config. Is librime installed?");

    let include_dirs: Vec<String> = lib.include_paths.iter().map(|p| p.to_string_lossy().into()).collect();

    // C++ standard library and other transitive deps are declared in rime.pc
    // (Libs.private), so pkg-config handles them automatically.

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
