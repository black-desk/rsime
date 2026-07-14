// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::path::{Path, PathBuf};

/// Emit `cargo:rerun-if-changed` for every file under `dir`, relative to `base`.
fn emit_rerun_for_dir(dir: &Path, base: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                emit_rerun_for_dir(&path, base);
            } else if let Ok(rel) = path.strip_prefix(base) {
                println!("cargo:rerun-if-changed={}", rel.display());
            }
        }
    }
}

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
    Err(concat!(
        "bundled-vcpkg feature requires vcpkg, but it was not found.\n",
        "Install vcpkg: https://vcpkg.io/en/getting-started.html\n",
        "Then either set VCPKG_ROOT or ensure the vcpkg executable is in PATH."
    )
    .to_string())
}

#[cfg(feature = "bundled-vcpkg")]
fn vcpkg_triplet() -> Result<String, String> {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    match (arch.as_str(), os.as_str()) {
        ("x86_64", "linux") => Ok("x64-linux".to_string()),
        ("aarch64", "linux") => Ok("arm64-linux".to_string()),
        ("x86_64", "macos") => Ok("x64-osx".to_string()),
        ("aarch64", "macos") => Ok("arm64-osx".to_string()),
        _ => Err(format!(
            "bundled-vcpkg feature does not support target {arch}-{os}"
        )),
    }
}

#[cfg(feature = "bundled-vcpkg")]
fn run_vcpkg_install(
    manifest_dir: &Path,
    vcpkg_root: &Path,
    target_dir: &Path,
) -> Result<PathBuf, String> {
    let vcpkg_bin = vcpkg_root.join("vcpkg");

    let triplet = vcpkg_triplet()?;

    let install_root = target_dir.join("vcpkg_installed");

    let output = std::process::Command::new(&vcpkg_bin)
        .args(["install", "--triplet", &triplet, "--allow-unsupported"])
        .arg(format!("--x-install-root={}", install_root.display()))
        .current_dir(manifest_dir)
        .env("VCPKG_ROOT", vcpkg_root)
        .output()
        .map_err(|e| format!("Failed to execute vcpkg: {e}"))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "vcpkg install failed with status: {}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
            output.status
        ));
    }

    let installed_dir = install_root.join(&triplet);
    if !installed_dir.exists() {
        return Err(format!(
            "vcpkg installed directory not found: {}",
            installed_dir.display()
        ));
    }

    Ok(installed_dir)
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Emit rerun-if directives for bindgen inputs (always relevant)
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=build.rs");
    let include_dir = manifest_dir.join("include");
    if include_dir.exists() {
        emit_rerun_for_dir(&include_dir, &manifest_dir);
    }

    // === bundled-vcpkg feature: install librime via vcpkg ===
    #[cfg(feature = "bundled-vcpkg")]
    {
        // Rerun when vcpkg config or overlay changes
        println!("cargo:rerun-if-changed=vcpkg.json");
        println!("cargo:rerun-if-changed=vcpkg-configuration.json");
        println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
        let overlay_dir = manifest_dir.join("vcpkg-overlay");
        if overlay_dir.exists() {
            emit_rerun_for_dir(&overlay_dir, &manifest_dir);
        }

        let vcpkg_root =
            find_vcpkg_root().expect("bundled-vcpkg feature requires vcpkg to be installed");

        // Derive target/ from OUT_DIR: target/<profile>/build/<hash>/out
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let target_dir = out_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        let installed_dir = run_vcpkg_install(&manifest_dir, &vcpkg_root, &target_dir)
            .expect("vcpkg install for librime failed");

        let pkg_config_dir = installed_dir.join("lib").join("pkgconfig");
        if pkg_config_dir.exists() {
            let existing = env::var("PKG_CONFIG_PATH").unwrap_or_default();
            let new_path = if existing.is_empty() {
                pkg_config_dir.to_string_lossy().into_owned()
            } else {
                // TODO: Windows uses `;` as path separator
                format!("{}:{}", pkg_config_dir.to_string_lossy(), existing)
            };
            // SAFETY: build.rs is single-threaded; setting an env var here is safe.
            unsafe {
                env::set_var("PKG_CONFIG_PATH", &new_path);
            }
        }
    }

    let lib = pkg_config::Config::new()
        .probe("rime")
        .expect("Failed to find librime via pkg-config. Is librime installed?");

    let include_dirs: Vec<String> = lib
        .include_paths
        .iter()
        .map(|p| p.to_string_lossy().into())
        .collect();

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
