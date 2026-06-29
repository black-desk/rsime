// SPDX-FileCopyrightText: 2026 Chen Linxian <me@black_desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

/// 构造一个以 `home` 为 HOME 的 `rsime` 子进程。
fn config_cmd(home: &std::path::Path) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("rsime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn config_set_then_get_bool() {
    // 同一个临时 HOME：set 写 user.yaml，get 读回。
    let temp = tempfile::tempdir().unwrap();

    config_cmd(temp.path())
        .args(["config", "set", "var/option/simplification", "true"])
        .assert()
        .success();

    config_cmd(temp.path())
        .args(["config", "get", "var/option/simplification"])
        .assert()
        .success()
        .stdout("true\n");
}

#[test]
fn config_set_then_get_int() {
    let temp = tempfile::tempdir().unwrap();
    config_cmd(temp.path())
        .args(["config", "set", "var/option/custom_int", "123"])
        .assert()
        .success();
    config_cmd(temp.path())
        .args(["config", "get", "var/option/custom_int"])
        .assert()
        .success()
        .stdout("123\n");
}

#[test]
fn config_set_then_get_string() {
    let temp = tempfile::tempdir().unwrap();
    config_cmd(temp.path())
        .args(["config", "set", "var/option/custom_str", "hello"])
        .assert()
        .success();
    config_cmd(temp.path())
        .args(["config", "get", "var/option/custom_str"])
        .assert()
        .success()
        .stdout("hello\n");
}
