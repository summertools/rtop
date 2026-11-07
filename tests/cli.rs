//! End-to-end CLI tests. `--dump` deliberately needs no terminal, which makes
//! the complete render path testable from CI.

use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rtop"))
}

#[test]
fn dump_renders_a_plain_frame() {
    let output = binary()
        .args(["--dump", "--width", "100", "--height", "30"])
        .env("NO_COLOR", "1")
        .output()
        .expect("run rtop");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtop"));
    assert!(stdout.contains("cpu"));
    assert!(stdout.contains("mem"));
    assert!(stdout.contains("proc"));
    assert!(!stdout.contains('\u{1b}'));
    assert!(stdout.lines().count() >= 24);
    for line in stdout.lines() {
        assert!(unicode_width::UnicodeWidthStr::width(line) <= 100);
    }
}

#[test]
fn presets_and_themes_are_accepted() {
    for preset in ["0", "1", "2", "3", "4"] {
        let output = binary()
            .args([
                "--dump",
                "--width",
                "120",
                "--height",
                "36",
                "--preset",
                preset,
                "--theme",
                "dracula",
                "--graph-symbol",
                "block",
            ])
            .env("NO_COLOR", "1")
            .output()
            .expect("run rtop");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.is_empty());
        assert!(!stdout.contains('\u{1b}'));
    }
}

#[test]
fn list_themes_works_without_a_terminal() {
    let output = binary().arg("--list-themes").output().expect("run rtop");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for theme in [
        "default",
        "dracula",
        "gruvbox",
        "nord",
        "catppuccin",
        "light",
        "tty",
    ] {
        assert!(stdout.contains(theme), "missing theme {theme}");
    }
}

#[test]
fn invalid_values_warn_but_still_render() {
    let output = binary()
        .args([
            "--dump",
            "--width",
            "80",
            "--height",
            "24",
            "--preset",
            "99",
            "--graph-symbol",
            "sparkles",
            "--update",
            "1",
        ])
        .env("NO_COLOR", "1")
        .output()
        .expect("run rtop");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("preset 99"));
    assert!(stderr.contains("graph symbol"));
    assert!(stderr.contains("update_ms"));
}

#[test]
fn tiny_dump_size_renders_the_hint() {
    let output = binary()
        .args(["--dump", "--width", "20", "--height", "8"])
        .env("NO_COLOR", "1")
        .output()
        .expect("run rtop");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("too small"));
}

#[test]
fn interactive_mode_without_a_terminal_fails_clearly() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtop"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("run rtop");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("terminal"), "stderr: {stderr}");
}

#[test]
fn explicit_config_file_is_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        "update_ms = 500\npreset = 1\ntheme = \"nord\"\nproc_sort = \"memory\"\n",
    )
    .unwrap();
    let output = binary()
        .args([
            "--config",
            config.to_str().unwrap(),
            "--dump",
            "--width",
            "100",
            "--height",
            "30",
        ])
        .env("NO_COLOR", "1")
        .output()
        .expect("run rtop");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("10 cores") || stdout.contains("cpu"));
}

#[test]
fn help_and_version_are_available() {
    let help = binary().arg("--help").output().expect("run help");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--dump"));

    let version = binary().arg("--version").output().expect("run version");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("rtop"));
}

#[test]
fn tty_dump_is_pure_ascii() {
    let output = binary()
        .args([
            "--dump", "--width", "100", "--height", "30", "--tty", "--preset", "4",
        ])
        .env("NO_COLOR", "1")
        .output()
        .expect("run rtop");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_ascii(), "non-ASCII bytes in --tty output");
    assert!(stdout.contains('#') || stdout.contains('+'));
}

#[test]
fn closing_the_output_pipe_is_not_a_failure() {
    let mut child = binary()
        .args(["--dump", "--width", "80", "--height", "24"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn rtop");
    drop(child.stdout.take());
    let status = child.wait().expect("wait for rtop");
    assert!(status.success());
}
