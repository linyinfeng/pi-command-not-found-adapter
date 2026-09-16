//! Every setting of `run`, defined once and filled from one figment stack:
//! the defaults, then the JSON files (`/etc/xdg` and `$XDG_CONFIG_HOME`,
//! which is where the Nix modules put theirs), then the `PI_COMMAND_NOT_FOUND_*`
//! variables. Nothing else defines them, so a shell only has to run
//! `run --shell <shell> -- <command>`.
//!
//! `PI_COMMAND_NOT_FOUND_CONFIG` replaces the two files with the one it
//! names, and an unknown key is an error so a typo is not silently ignored.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use figment::Figment;
use figment::providers::{Env, Format, Json, Serialized};
use serde::{Deserialize, Serialize};

/// Keys are the option names in kebab case: `model`, `pi-args`,
/// `system-prompt-file`, `tool-lines`, …
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Config {
    pub pi: String,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub pi_args: Vec<String>,
    pub session_root: Option<PathBuf>,
    pub system_prompt_file: Vec<PathBuf>,
    pub mcat: String,
    pub width: Option<usize>,
    pub retries: u32,
    pub tool_lines: usize,
    pub timeout: u64,
    pub trace: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pi: "pi".into(),
            model: None,
            thinking: None,
            pi_args: Vec::new(),
            session_root: None,
            system_prompt_file: Vec::new(),
            mcat: "mcat".into(),
            width: None,
            retries: 2,
            tool_lines: 5,
            timeout: 600,
            trace: None,
        }
    }
}

/// The config files a user or a module can drop, lowest priority first:
/// `$XDG_CONFIG_DIRS` (default `/etc/xdg`), then `$XDG_CONFIG_HOME`.
fn config_files(config_dirs: &str, config_home: Option<&Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in config_dirs.split(':').filter(|dir| !dir.is_empty()) {
        files.push(Path::new(dir).join("pi-command-not-found/config.json"));
    }
    if let Some(home) = config_home {
        files.push(home.join("pi-command-not-found/config.json"));
    }
    files
}

/// The files to merge: the named one alone if there is one, otherwise every
/// one of `config_files` that exists.
fn files() -> Result<Vec<PathBuf>> {
    let Some(named) = env::var_os("PI_COMMAND_NOT_FOUND_CONFIG") else {
        return Ok(config_files(
            &env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".into()),
            dirs::config_dir().as_deref(),
        )
        .into_iter()
        .filter(|path| path.is_file())
        .collect());
    };
    let named = PathBuf::from(named);
    if !named.is_file() {
        bail!(
            "the config file {} named by PI_COMMAND_NOT_FOUND_CONFIG is not a file",
            named.display()
        );
    }
    Ok(vec![named])
}

/// The two settings that are lists: their variables hold the items, so they
/// cannot go through `Env` like the plain ones.
fn list_vars(
    pi_args: Option<String>,
    prompt_files: Option<String>,
) -> Serialized<BTreeMap<&'static str, Vec<String>>> {
    let split = |value: String, sep: char| value.split(sep).map(str::to_string).collect::<Vec<_>>();
    let mut lists = BTreeMap::new();
    if let Some(value) = pi_args {
        lists.insert("pi-args", split(value, '\n'));
    }
    if let Some(value) = prompt_files {
        lists.insert("system-prompt-file", split(value, ':'));
    }
    Serialized::defaults(lists)
}

/// Read the configuration: the defaults, the XDG files, the one given to
/// `--config` (which wins over the XDG ones), then the environment.
pub fn load(extra: Option<&Path>) -> Result<Config> {
    let mut files = files()?;
    if let Some(extra) = extra {
        if !extra.is_file() {
            bail!(
                "the config file {} given to --config is not a file",
                extra.display()
            );
        }
        files.push(extra.to_path_buf());
    }
    let mut figment = Figment::new();
    for path in &files {
        figment = figment.merge(Json::file(path));
    }
    let lists = list_vars(
        env::var("PI_COMMAND_NOT_FOUND_PI_ARGS").ok(),
        env::var("PI_COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE").ok(),
    );
    let named: Vec<String> = files
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    figment
        .merge(lists)
        .merge(
            // Not every variable in the namespace is a setting, so `Env` has
            // to leave the others alone: `CONFIG` is a path, `SHELL` and
            // `SESSION_ID` are arguments of the call (read by clap), and the
            // two lists are split by `list_vars` instead of being injected as
            // their raw strings.
            Env::prefixed("PI_COMMAND_NOT_FOUND_")
                .ignore(&[
                    "CONFIG",
                    "SHELL",
                    "SESSION_ID",
                    "PI_ARGS",
                    "SYSTEM_PROMPT_FILE",
                ])
                .map(|key| key.as_str().replace('_', "-").into()),
        )
        .extract()
        .with_context(|| match named.is_empty() {
            true => "cannot read the configuration".to_string(),
            false => format!("cannot read the configuration from {}", named.join(", ")),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_file_comes_first_and_the_user_file_last() {
        let files = config_files("/etc/xdg:/run/xdg", Some(Path::new("/home/user/.config")));
        assert_eq!(
            files,
            [
                PathBuf::from("/etc/xdg/pi-command-not-found/config.json"),
                PathBuf::from("/run/xdg/pi-command-not-found/config.json"),
                PathBuf::from("/home/user/.config/pi-command-not-found/config.json"),
            ]
        );
        assert_eq!(
            config_files("", None),
            Vec::<PathBuf>::new(),
            "an empty search path has no files"
        );
    }

    /// What `load` does with its providers, without touching the environment.
    #[test]
    fn later_layers_win_and_the_rest_survives() {
        let merged: Config = Figment::new()
            .merge(Json::string(
                r#"{"model":"system","retries":1,"pi-args":["--system"]}"#,
            ))
            .merge(Json::string(r#"{"model":"user","mcat":"user-mcat"}"#))
            .merge(list_vars(Some("--env".into()), None))
            .extract()
            .unwrap();
        assert_eq!(merged.model.as_deref(), Some("user"));
        assert_eq!(merged.mcat, "user-mcat");
        assert_eq!(merged.retries, 1);
        assert_eq!(merged.pi_args, ["--env"]);
        assert_eq!(merged.pi, "pi", "unset settings keep their default");
    }

    #[test]
    fn an_unknown_key_is_an_error() {
        let unknown = Figment::new()
            .merge(Json::string(r#"{"models":"typo"}"#))
            .extract::<Config>();
        assert!(unknown.is_err(), "a typo must not be ignored");
    }

    #[test]
    fn a_list_variable_is_split_like_the_adapter_reads_it() {
        let lists: Config = Figment::new()
            .merge(list_vars(
                Some("--one\ntwo words".into()),
                Some("/a.md:/b/c.md".into()),
            ))
            .extract()
            .unwrap();
        assert_eq!(lists.pi_args, ["--one", "two words"]);
        assert_eq!(
            lists.system_prompt_file,
            [PathBuf::from("/a.md"), PathBuf::from("/b/c.md")]
        );
    }
}
