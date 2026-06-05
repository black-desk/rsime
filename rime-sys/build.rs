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
        if path.join("vcpkg").exists() || path.join("vcpkg.exe").exists() {
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
    let env_var = if env::var("CARGO_CFG_TARGET_ENV").unwrap() == "msvc" {
        "windows"
    } else {
        ""
    };

    match (arch.as_str(), os.as_str(), env_var) {
        ("x86_64", "linux", _) => "x64-linux".to_string(),
        ("aarch64", "linux", _) => "arm64-linux".to_string(),
        ("x86_64", "macos", _) => "x64-osx".to_string(),
        ("aarch64", "macos", _) => "arm64-osx".to_string(),
        ("x86_64", "windows", "windows") => "x64-windows-static".to_string(),
        ("aarch64", "windows", "windows") => "arm64-windows-static".to_string(),
        _ => format!("unknown triplet: arch={arch}, os={os}"),
    }
}

#[cfg(feature = "bundled-vcpkg")]
fn run_vcpkg_install(manifest_dir: &Path, vcpkg_root: &Path) -> Result<PathBuf, String> {
    let vcpkg_bin = if vcpkg_root.join("vcpkg").exists() {
        vcpkg_root.join("vcpkg")
    } else {
        vcpkg_root.join("vcpkg.exe")
    };

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
    let mut include_dirs: Vec<String> = Vec::new();

    // === bundled-vcpkg feature: install librime via vcpkg ===
    #[cfg(feature = "bundled-vcpkg")]
    let is_static: bool = {
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
        true
    };

    #[cfg(not(feature = "bundled-vcpkg"))]
    let mut is_static: bool = false;

    // Try pkg-config first
    if pkg_config::probe_library("rime").is_ok() {
        if let Ok(lib) = pkg_config::Config::new().probe("rime") {
            include_dirs = lib.include_paths.iter().map(|p| p.to_string_lossy().into()).collect();
            #[cfg(not(feature = "bundled-vcpkg"))]
            {
                is_static = lib.link_paths.iter().any(|dir| {
                    dir.join("librime.a").exists() || dir.join("librime-static.a").exists()
                });
            }
        }
    }

    // Fallback to environment variables or defaults
    if include_dirs.is_empty() {
        let include_dir =
            env::var("RIME_INCLUDE_DIR").unwrap_or_else(|_| "/usr/include".to_owned());
        let lib_dir = env::var("RIME_LIB_DIR").unwrap_or_else(|_| "/usr/lib".to_owned());

        println!("cargo:rustc-link-search={lib_dir}");
        println!("cargo:rustc-link-lib=rime");

        #[cfg(not(feature = "bundled-vcpkg"))]
        {
            is_static = PathBuf::from(&lib_dir).join("librime.a").exists();
        }

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
