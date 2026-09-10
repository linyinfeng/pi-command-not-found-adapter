use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_command-not-found-agent");

/// A fake `pi --mode rpc`: turn N answers with `turns[N]`.
fn fake_pi(dir: &Path, turns: &[&str]) -> PathBuf {
    let path = dir.join("pi");
    let mut script = format!("#!{}\nturn=0\nwhile IFS= read -r line; do\n", shell());
    script.push_str("  case \"$line\" in *'\"prompt\"'*)\n");
    script.push_str("    turn=$((turn+1))\n");
    script.push_str("    echo '{\"type\":\"response\",\"command\":\"prompt\",\"success\":true}'\n");
    for (index, text) in turns.iter().enumerate() {
        let message = serde_json::json!({
            "type": "message_end",
            "message": {"role": "assistant", "content": [{"type": "text", "text": text}]},
        });
        script.push_str(&format!(
            "    if [ \"$turn\" -eq {} ]; then echo '{}'; fi\n",
            index + 1,
            message
        ));
    }
    script.push_str("    echo '{\"type\":\"agent_settled\"}'\n  ;; esac\ndone\n");
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn run(dir: &Path, pi: &Path, extra: &[&str]) -> Output {
    Command::new(BIN)
        .arg("run")
        .arg("--shell")
        .arg("bash")
        .arg("--pi")
        .arg(pi)
        .arg("--session-root")
        .arg(dir.join("sessions"))
        .arg("--mcat")
        .arg("/nonexistent-mcat")
        .args(extra)
        .arg("--")
        .arg("cowsay")
        .arg("hi")
        .output()
        .unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cna-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn retries_then_prints_the_source() {
    let dir = temp_dir("retry");
    let pi = fake_pi(
        &dir,
        &[
            "I think you want cowsay.",
            r#"{"markdown":"note","source":"nix shell nixpkgs#cowsay -c cowsay hi"}"#,
        ],
    );
    let output = run(&dir, &pi, &[]);
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "nix shell nixpkgs#cowsay -c cowsay hi"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("note"), "{stderr}");
    assert!(stderr.contains("⚡ nix shell nixpkgs#cowsay"), "{stderr}");

    let history = only_history(&dir);
    assert_eq!(read(&history.join("markdown")), "note");
    assert_eq!(
        read(&history.join("source")),
        "nix shell nixpkgs#cowsay -c cowsay hi"
    );
    assert_eq!(read(&history.join("input")), "cowsay hi");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn omits_the_source_when_there_is_none() {
    let dir = temp_dir("nosource");
    let pi = fake_pi(&dir, &[r#"{"markdown":"just a note"}"#]);
    let output = run(&dir, &pi, &[]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("just a note"), "{stderr}");
    assert!(!stderr.contains('⚡'), "{stderr}");
    let history = only_history(&dir);
    assert_eq!(read(&history.join("source")), "");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fails_after_exhausting_retries() {
    let dir = temp_dir("fail");
    let pi = fake_pi(&dir, &["nope", "still nope", "nope again"]);
    let output = run(&dir, &pi, &[]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert!(String::from_utf8_lossy(&output.stderr).contains("command-not-found:"));
    let _ = fs::remove_dir_all(&dir);
}

fn only_history(dir: &Path) -> PathBuf {
    let sessions = dir.join("sessions");
    let session = fs::read_dir(&sessions)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let history = session.join("history");
    fs::read_dir(&history)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// The fake pi is a script, so the shebang must name a shell that exists —
/// inside a Nix build sandbox there is no `/bin/sh`.
fn shell() -> String {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        for name in ["bash", "sh"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return candidate.display().to_string();
            }
        }
    }
    "/bin/sh".into()
}
