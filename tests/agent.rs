use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_command-not-found-agent");

/// A fake `pi --mode rpc`: turn N answers with `turns[N]`. With `stale`,
/// every prompt is preceded by a leftover `message_end` from an earlier
/// conversation, as a resumed session might replay.
fn fake_pi(dir: &Path, turns: &[&str], stale: bool) -> PathBuf {
    let path = dir.join("pi");
    let mut script = format!(
        "#!{}\nprintf '%s\\n' \"$@\" > {}/pi-args\nturn=0\nwhile IFS= read -r line; do\n",
        shell(),
        dir.display()
    );
    script.push_str("  case \"$line\" in *'\"prompt\"'*)\n");
    script.push_str("    turn=$((turn+1))\n");
    if stale {
        let stale = serde_json::json!({
            "type": "message_end",
            "message": {"role": "assistant", "content": [{"type": "text", "text": "{\"source\":\"echo stale\"}"}]},
        });
        script.push_str(&format!("    echo '{}'\n", stale));
    }
    script.push_str("    echo 'pi-stderr-marker' >&2\n");
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

/// The adapter takes its settings from the environment (and the config file),
/// so the harness hands them over that way too, exactly like a shell does.
fn run(dir: &Path, pi: &Path) -> Output {
    run_with(dir, pi, &[])
}

fn run_with(dir: &Path, pi: &Path, envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(BIN);
    command
        .arg("run")
        .arg("--shell")
        .arg("bash")
        .env("PI_COMMAND_NOT_FOUND_PI", pi)
        .env("PI_COMMAND_NOT_FOUND_SESSION_ROOT", dir.join("sessions"))
        .env("PI_COMMAND_NOT_FOUND_MCAT", "/nonexistent-mcat")
        .arg("--")
        .arg("cowsay")
        .arg("hi");
    for (name, value) in envs {
        command.env(name, value);
    }
    command.output().unwrap()
}

/// What pi was started with, as the fake pi recorded it.
fn pi_args(dir: &Path) -> Vec<String> {
    read(&dir.join("pi-args"))
        .lines()
        .map(str::to_string)
        .collect()
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
        false,
    );
    let output = run(&dir, &pi);
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
    let pi = fake_pi(&dir, &[r#"{"markdown":"just a note"}"#], false);
    let output = run(&dir, &pi);
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
    let pi = fake_pi(&dir, &["nope", "still nope", "nope again"], false);
    let output = run(&dir, &pi);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert!(String::from_utf8_lossy(&output.stderr).contains("command-not-found:"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ignores_messages_before_the_prompt_is_acknowledged() {
    // A stale answer from an earlier conversation must not become the verdict;
    // with nothing else to parse the run has to fail instead.
    let dir = temp_dir("stale");
    let pi = fake_pi(&dir, &["nothing useful this turn"], true);
    let output = run(&dir, &pi);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "{output:?}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("stale"),
        "{output:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_config_file_fills_the_options() {
    // The Nix modules hand over a JSON file and nothing else, so its values
    // have to reach the pi command line.
    let dir = temp_dir("config");
    let pi = fake_pi(&dir, &[r#"{"markdown":"note","source":"echo hi"}"#], false);
    let config = dir.join("config.json");
    fs::write(
        &config,
        r#"{"model":"deepseek/x","pi-args":["--verbose","two words"],"retries":2}"#,
    )
    .unwrap();
    let output = run_with(
        &dir,
        &pi,
        &[("PI_COMMAND_NOT_FOUND_CONFIG", &config.display().to_string())],
    );
    assert!(output.status.success(), "{output:?}");
    let args = pi_args(&dir);
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--model", "deepseek/x"]),
        "{args:?}"
    );
    assert!(args.iter().any(|arg| arg == "--verbose"), "{args:?}");
    assert!(args.iter().any(|arg| arg == "two words"), "{args:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_command_line_wins_over_the_config_file() {
    let dir = temp_dir("config-flag");
    let pi = fake_pi(&dir, &[r#"{"markdown":"note","source":"echo hi"}"#], false);
    let config = dir.join("config.json");
    fs::write(&config, r#"{"model":"from-the-file"}"#).unwrap();
    let output = run_with(
        &dir,
        &pi,
        &[
            ("PI_COMMAND_NOT_FOUND_CONFIG", &config.display().to_string()),
            ("PI_COMMAND_NOT_FOUND_MODEL", "from-the-environment"),
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let args = pi_args(&dir);
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--model", "from-the-environment"]),
        "{args:?}"
    );
    assert!(!args.iter().any(|arg| arg == "from-the-file"), "{args:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_list_variable_is_split_into_arguments() {
    // `PI_COMMAND_NOT_FOUND_PI_ARGS` is the list setting's variable, and the
    // generic variable layer must not also claim it as a plain string.
    let dir = temp_dir("env-list");
    let pi = fake_pi(&dir, &[r#"{"markdown":"note","source":"echo hi"}"#], false);
    let output = run_with(
        &dir,
        &pi,
        &[("PI_COMMAND_NOT_FOUND_PI_ARGS", "--one\ntwo words")],
    );
    assert!(output.status.success(), "{output:?}");
    let args = pi_args(&dir);
    assert!(args.iter().any(|arg| arg == "--one"), "{args:?}");
    assert!(args.iter().any(|arg| arg == "two words"), "{args:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_prompt_file_variable_is_split_on_colons() {
    let dir = temp_dir("env-prompt");
    let first = dir.join("first.md");
    let second = dir.join("second.md");
    fs::write(&first, "the first marker\n").unwrap();
    fs::write(&second, "the second marker\n").unwrap();
    let prompt = Command::new(BIN)
        .args(["system-prompt"])
        .env(
            "PI_COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE",
            format!("{}:{}", first.display(), second.display()),
        )
        .output()
        .unwrap();
    assert!(prompt.status.success(), "{prompt:?}");
    let text = String::from_utf8_lossy(&prompt.stdout);
    assert!(text.contains("the first marker"), "first prompt missing");
    assert!(text.contains("the second marker"), "second prompt missing");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn forwards_pi_s_own_diagnostics() {
    // pi's stderr is piped and printed through the progress block: a child
    // writing to the terminal behind the block is what used to leave stale
    // spinner frames on screen.
    let dir = temp_dir("stderr");
    let pi = fake_pi(&dir, &[r#"{"markdown":"note","source":"echo hi"}"#], false);
    let output = run(&dir, &pi);
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("pi-stderr-marker"), "{stderr}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_prints_the_layered_settings() {
    let dir = temp_dir("config-print");
    let file = dir.join("config.json");
    fs::write(&file, r#"{"model":"from-file","retries":7}"#).unwrap();
    let output = Command::new(BIN)
        .args(["config"])
        .env("PI_COMMAND_NOT_FOUND_CONFIG", &file)
        .env("PI_COMMAND_NOT_FOUND_THINKING", "from-environment")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let printed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(printed["model"], "from-file", "the file's value");
    assert_eq!(printed["retries"], 7, "the file's number stays a number");
    assert_eq!(printed["thinking"], "from-environment", "the variable wins");
    assert_eq!(
        printed["pi"], "pi",
        "an untouched setting shows its default"
    );
    assert!(printed["width"].is_null(), "a setting nobody made is null");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_call_s_own_variables_are_not_settings() {
    // `PI_COMMAND_NOT_FOUND_SHELL` and `…_SESSION_ID` belong to the call, so
    // the settings layer has to step over them rather than reject them.
    let output = Command::new(BIN)
        .args(["config"])
        .env("PI_COMMAND_NOT_FOUND_SHELL", "bash")
        .env("PI_COMMAND_NOT_FOUND_SESSION_ID", "a-session")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let printed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(printed.get("shell").is_none(), "{printed}");
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
