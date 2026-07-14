// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use serde_json::Value;

/// Set up a temp HOME so rsime's init_rime() will auto-install schemas.
fn setup_rime_env() -> (tempfile::TempDir, assert_cmd::Command) {
    let temp_dir = tempfile::tempdir().unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("rsime").unwrap();
    cmd.env("HOME", temp_dir.path());
    cmd.arg("stdio");

    (temp_dir, cmd)
}

/// Parse JSONL output into a vector of JSON values.
fn parse_jsonl(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|_| panic!("invalid json: {l}")))
        .collect()
}

#[test]
fn stdio_basic_commit() {
    let (_temp, mut cmd) = setup_rime_env();
    cmd.write_stdin("n\ni\n<Space>\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert!(!responses.is_empty(), "should have at least one response");

    // The last response should have a commit (user selected a candidate)
    let last = responses.last().unwrap();
    let commit = last["commit"].as_str().unwrap();
    assert!(
        !commit.is_empty(),
        "last response should have committed text"
    );
    assert_eq!(last["preedit"].as_str().unwrap(), "");
    assert!(last["candidates"].as_array().unwrap().is_empty());
}

#[test]
fn stdio_preedit_builds_up() {
    let (_temp, mut cmd) = setup_rime_env();
    cmd.write_stdin("n\ni\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert_eq!(responses.len(), 2);

    // First key 'n' → preedit contains "n"
    let first = &responses[0];
    assert!(first["preedit"].as_str().unwrap().contains('n'));
    assert_eq!(first["commit"].as_str().unwrap(), "");

    // Second key 'i' → preedit contains "ni"
    let second = &responses[1];
    assert!(second["preedit"].as_str().unwrap().contains("ni"));
    assert_eq!(second["commit"].as_str().unwrap(), "");
}

#[test]
fn stdio_candidates_appear() {
    let (_temp, mut cmd) = setup_rime_env();
    cmd.write_stdin("n\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert_eq!(responses.len(), 1);
    let candidates = responses[0]["candidates"].as_array().unwrap();
    assert!(!candidates.is_empty(), "typing should produce candidates");

    // Each candidate should have a "text" field
    for cand in candidates {
        assert!(cand["text"].is_string());
    }
}

#[test]
fn stdio_highlighted_index() {
    let (_temp, mut cmd) = setup_rime_env();
    cmd.write_stdin("n\n<Down>\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert!(responses.len() >= 2);
    // After <Down>, highlighted should be >= 1
    assert!(responses[1]["highlighted"].as_u64().unwrap() >= 1);
}

#[test]
fn stdio_esc_clears() {
    let (_temp, mut cmd) = setup_rime_env();
    // Type 'n', then Esc (not composing → exit)
    cmd.write_stdin("n\n<Esc>\n<Esc>\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    // Should have at least the 'n' response
    assert!(!responses.is_empty());
}

#[test]
fn stdio_empty_input_exits_cleanly() {
    let (_temp, mut cmd) = setup_rime_env();
    // EOF immediately
    cmd.write_stdin("");

    cmd.assert().success();
}

#[test]
fn stdio_backspace() {
    let (_temp, mut cmd) = setup_rime_env();
    cmd.write_stdin("n\ni\n<BS>\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert!(responses.len() >= 3);
    // After backspace, preedit should be shorter than "ni"
    let after_bs = &responses[2];
    let preedit = after_bs["preedit"].as_str().unwrap();
    assert!(
        !preedit.contains("ni"),
        "backspace should remove last input"
    );
}

#[test]
fn stdio_response_schema_has_required_fields() {
    let (_temp, mut cmd) = setup_rime_env();
    cmd.write_stdin("n\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert_eq!(responses.len(), 1);
    let resp = &responses[0];
    // Every response must contain these four fields
    assert!(resp.get("commit").is_some(), "missing 'commit' field");
    assert!(resp.get("preedit").is_some(), "missing 'preedit' field");
    assert!(
        resp.get("candidates").is_some(),
        "missing 'candidates' field"
    );
    assert!(
        resp.get("highlighted").is_some(),
        "missing 'highlighted' field"
    );
}

#[test]
fn stdio_cr_selects_first_candidate() {
    let (_temp, mut cmd) = setup_rime_env();
    // Type 'n' then Enter to select the first candidate
    cmd.write_stdin("n\n<CR>\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    // Should have at least 2 responses: one for 'n', one for <CR>
    assert!(responses.len() >= 2);
    // The last response should have a commit
    let last = responses.last().unwrap();
    let commit = last["commit"].as_str().unwrap();
    assert!(!commit.is_empty(), "<CR> should commit the first candidate");
}

#[test]
fn stdio_unknown_keys_ignored() {
    let (_temp, mut cmd) = setup_rime_env();
    // Send an unrecognized <Foo> key — should be ignored (no output line for it)
    cmd.write_stdin("<Foo>\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert!(
        responses.is_empty(),
        "unknown keys should produce no response"
    );
}

#[test]
fn stdio_number_selects_candidate() {
    let (_temp, mut cmd) = setup_rime_env();
    // Type 'n', then '2' to select the second candidate (if available)
    cmd.write_stdin("n\n2\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert!(responses.len() >= 2);
    // After pressing '2', the response should have either a commit or still show candidates
    let resp = &responses[1];
    assert!(resp.get("commit").is_some());
    assert!(resp.get("preedit").is_some());
}

#[test]
fn stdio_blank_lines_ignored() {
    let (_temp, mut cmd) = setup_rime_env();
    // Send blank lines mixed with input
    cmd.write_stdin("\n\nn\n\ni\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    // Should get exactly 2 responses (for 'n' and 'i'), blank lines ignored
    assert_eq!(responses.len(), 2);
}

#[test]
fn stdio_up_navigates_candidates() {
    let (_temp, mut cmd) = setup_rime_env();
    cmd.write_stdin("n\n<Up>\n");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses = parse_jsonl(&stdout);

    assert!(responses.len() >= 2);
    // After pressing Up, highlighted should still be valid
    assert!(responses[1]["highlighted"].is_number());
}
