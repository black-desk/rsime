// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use assert_cmd::Command;

fn shell_init_stdout(shell: &str) -> String {
    let mut cmd = Command::cargo_bin("rsime").unwrap();
    cmd.args(["shell-init", shell, "--bind", r"\ei"]);
    String::from_utf8_lossy(
        &cmd.assert()
            .success()
            .get_output()
            .clone()
            .stdout,
    )
    .to_string()
}

#[test]
fn zsh_binding_passes_prompt() {
    let out = shell_init_stdout("zsh");
    // 绑定用 (e)+(%) 完整渲染 prompt（含 prompt_subst，支持 Powerlevel10k），
    // 渲染结果存入 $_rp 后传给 RSIME_PROMPT。
    assert!(
        out.contains(r#"RSIME_PROMPT="$_rp""#),
        "zsh binding should pass RSIME_PROMPT from rendered $_rp, got:\n{out}"
    );
    assert!(out.contains("${(%):-${(e)PROMPT}}"));
    assert!(out.contains(r#"RSIME_READLINE_LINE="$BUFFER""#));
}

#[test]
fn fish_binding_passes_prompt() {
    let out = shell_init_stdout("fish");
    assert!(out.contains("RSIME_PROMPT=(fish_prompt)"));
    assert!(out.contains("RSIME_READLINE_LINE=(commandline)"));
    // fish 做差分重绘，需要 rsime 退出时把覆盖掉的 prompt 画回去，故设置 RSIME_RESTORE_PROMPT=1。
    assert!(
        out.contains("RSIME_RESTORE_PROMPT=1"),
        "fish binding should set RSIME_RESTORE_PROMPT=1 (rsime restores the prompt only when asked), got:\n{out}"
    );
}

/// bash 的 shell-init 会先做版本门控（系统 bash < 4.4 会 bail）。
/// macOS 自带 bash 3.2 → 跳过；bash ≥ 4.4 → 断言绑定含 RSIME_PROMPT。
#[test]
fn bash_binding_passes_prompt() {
    let ver = std::process::Command::new("bash")
        .arg("-c")
        .arg("echo \"${BASH_VERSINFO[0]}.${BASH_VERSINFO[1]}\"")
        .output();
    let ok = ver
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let mut p = v.split('.');
            let maj = p.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let min = p.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            (maj, min) >= (4, 4)
        })
        .unwrap_or(false);
    if !ok {
        eprintln!("skip bash binding test: system bash < 4.4");
        return;
    }
    let out = shell_init_stdout("bash");
    assert!(
        out.contains(r#"RSIME_PROMPT="${PS1@P}""#),
        "bash binding should pass RSIME_PROMPT, got:\n{out}"
    );
    // bash 退出后会全量重绘 prompt（rl_forced_update_display），不应设置
    // RSIME_RESTORE_PROMPT——否则 rsime 画一遍再被 readline 画一遍会重复。
    assert!(
        !out.contains("RSIME_RESTORE_PROMPT"),
        "bash binding must NOT set RSIME_RESTORE_PROMPT (bash repaints itself), got:\n{out}"
    );
}
