// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use assert_cmd::Command;

fn shell_init_stdout(shell: &str) -> String {
    let mut cmd = Command::cargo_bin("rsime").unwrap();
    cmd.args(["shell-init", shell, "--bind", r"\ei"]);
    String::from_utf8_lossy(&cmd.assert().success().get_output().clone().stdout).to_string()
}

/// draw-below 模式下，rsime 不再读取任何 shell 上下文：三个 shell 的绑定里都不应再
/// 出现任何 RSIME_* 环境变量，只跑 `rsime tui` 并用各 shell 原生变量把 output 插到光标处。
#[test]
fn no_shell_context_env_vars_in_any_binding() {
    for shell in ["bash", "zsh", "fish"] {
        let out = shell_init_stdout(shell);
        for var in [
            "RSIME_PROMPT",
            "RSIME_RESTORE_PROMPT",
            "RSIME_READLINE_LINE",
            "RSIME_READLINE_POINT",
        ] {
            assert!(
                !out.contains(var),
                "{shell} binding must not reference {var}, got:\n{out}"
            );
        }
    }
}

#[test]
fn bash_binding_splices_output_at_cursor() {
    let out = shell_init_stdout("bash");
    assert!(
        out.contains("bind -x"),
        "bash binding should use bind -x, got:\n{out}"
    );
    assert!(
        out.contains("rsime tui"),
        "bash binding should run rsime tui, got:\n{out}"
    );
    assert!(
        out.contains(r#"READLINE_LINE="${READLINE_LINE:0:$READLINE_POINT}$output${READLINE_LINE:$READLINE_POINT}""#),
        "bash binding should splice $output at $READLINE_POINT, got:\n{out}"
    );
    assert!(
        out.contains("READLINE_POINT=$(( READLINE_POINT + ${#output} ))"),
        "bash binding should advance READLINE_POINT, got:\n{out}"
    );
}

#[test]
fn zsh_binding_appends_to_lbuffer() {
    let out = shell_init_stdout("zsh");
    assert!(
        out.contains("zle -N"),
        "zsh binding should register a widget, got:\n{out}"
    );
    assert!(
        out.contains("rsime tui"),
        "zsh binding should run rsime tui, got:\n{out}"
    );
    assert!(
        out.contains(r#"LBUFFER+="$output""#),
        "zsh binding should append $output to LBUFFER, got:\n{out}"
    );
    assert!(
        out.contains("zle reset-prompt"),
        "zsh binding should call zle reset-prompt, got:\n{out}"
    );
}

#[test]
fn fish_binding_inserts_via_commandline_and_repaints() {
    let out = shell_init_stdout("fish");
    assert!(
        out.contains("rsime tui"),
        "fish binding should run rsime tui, got:\n{out}"
    );
    assert!(
        out.contains(r#"commandline --insert "$output""#),
        "fish binding should insert $output via commandline --insert, got:\n{out}"
    );
    // rsime 在 prompt 下方画屏会扰乱 fish 的差分重绘，必须用 commandline -f repaint 强制全量
    // 重绘（与 zsh 的 reset-prompt、bash 的 rl_forced_update_display 对应；fzf 的 fish 绑定
    // 同样以此收尾）。缺失会导致提交后 prompt 渲染错乱。
    assert!(
        out.contains("commandline -f repaint"),
        "fish binding must end with `commandline -f repaint` to force a full redraw after rsime disturbs the screen, got:\n{out}"
    );
}
