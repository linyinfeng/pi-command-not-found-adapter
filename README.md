# pi-command-not-found-adapter

> [!NOTE]
> This project was developed with LLM assistance.

A shell `command-not-found` handler that asks
[pi](https://github.com/earendil-works/pi-mono) for shell code, shows the
agent's work, and hands the code back to the shell to source.

```
$ sl
🔧bash: command -v sl
🔧bash: nix eval --raw nixpkgs#sl.name
`sl` isn't installed; it is in nixpkgs.
⚡ nix shell nixpkgs#sl -c sl
────────────────────────────────────────────────────────────────────────
```

The binary is the whole agent: it spawns `pi --mode rpc`, streams one
conversation per shell session, renders the progress block itself, and
prints the answer's `source` on stdout for the caller to `eval` — so the
code runs with the user's environment, not in a fresh process (bash and zsh
still eval it inside the hook's own subshell, see [Shell integration](#shell-integration)).

## Installation

### Generic

1. Install the `command-not-found-adapter` binary on `PATH` (Nix machine:
   `nix profile install .#` from this flake, or on any machine
   `cargo install --path .`).
2. Source the provided script for your shell — see the
   [Shell integration](#shell-integration) table:

```sh
source <path>/shell/bash.sh     # bash
source <path>/shell/zsh.zsh     # zsh
source <path>/shell/fish.fish   # fish
source <path>/shell/nushell.nu  # nushell
```

### Nix

The flake provides a NixOS module and a home-manager module, each of
which checks the shells enabled on its own layer and hooks those:

```nix
# configuration.nix
{ inputs, ... }: {
  # the package is not in nixpkgs; the overlay makes pkgs.* see it
  nixpkgs.overlays = [ inputs.pi-command-not-found-adapter.overlays.default ];
  imports = [ inputs.pi-command-not-found-adapter.nixosModules.default ];
  programs.pi-command-not-found-adapter = {
    enable = true;
    model = "deepseek/deepseek-v4-flash";
    thinking = "medium";
  };
}
```

```nix
# home-manager configuration (with home-manager's own pkgs)
{ inputs, ... }: {
  nixpkgs.overlays = [ inputs.pi-command-not-found-adapter.overlays.default ];
  imports = [ inputs.pi-command-not-found-adapter.homeManagerModules.default ];
  programs.pi-command-not-found-adapter.enable = true;
}
```

When home-manager runs with the global pkgs (`useGlobalPkgs`), the overlay
belongs on the NixOS side instead.

Both modules do the same thing per shell — source the hook — each gated on
that shell being enabled there (`programs.bash.enable`,
`programs.zsh.enable`, `programs.fish.enable`, `programs.nushell.enable`).
NixOS uses the shells' `interactiveShellInit` and a generated nushell
autoload, home-manager uses `initExtra`, `initContent`,
`interactiveShellInit` and `extraConfig`.
`programs.pi-command-not-found-adapter.package` defaults to
`pkgs.pi-command-not-found-adapter` (provided by the overlay above);
override it to use a different build.

Every knob of `run` is an option: `pi`, `model`, `thinking`, `piArgs`,
`sessionRoot`, `systemPromptFile`, `mcat`, `width`, `retries`,
`toolLines`, `timeout`, `trace`. They are written to one JSON file — the
NixOS module puts it in `/etc/xdg`, home-manager in the user's own config
directory — so nothing at all is exported to the shell; [below](#run-options)
is how that file layers with one the user writes. An unset option leaves
the adapter's own default alone, except `systemPromptFile`, which the
modules point at the Nix prompt (`prompts/nix.md`); set it to
`passthru.prompts.base` for the generic one.

**The index `nix-locate` reads.** The Nix prompt's first step needs the
nix-index file database, and building it locally takes hours:
[nix-index-database](https://github.com/nix-community/nix-index-database)
ships a prebuilt one, refreshed from Hydra's cache.

```nix
# flake.nix
nix-index-database.url = "github:nix-community/nix-index-database";
nix-index-database.inputs.nixpkgs.follows = "nixpkgs";
```

```nix
# configuration.nix
imports = [ inputs.nix-index-database.nixosModules.default ];
```

```nix
# home-manager configuration
imports = [ inputs.nix-index-database.homeModules.default ];
# ours is the hook, so keep nix-index's own shell integration off
programs.nix-index.enableBashIntegration = false;
programs.nix-index.enableZshIntegration = false;
programs.nix-index.enableFishIntegration = false;
programs.nix-index.enableNushellIntegration = false;
```

Importing it is enough on NixOS: it installs `nix-index` with the prebuilt
database and keeps `programs.command-not-found.enable` off — which the
modules here set as well, so exactly one hook answers a missing command.
Without any index, the prompt falls through to `nix search`.

## CLI

```sh
command-not-found-agent [--config <FILE>] run --shell <SHELL> [--session-id <ID>] -- <command> [args...]
command-not-found-agent [--config <FILE>] session-id
command-not-found-agent [--config <FILE>] system-prompt
command-not-found-agent [--config <FILE>] config
```

`run` is the handler: it takes the command line after `--` and prints the
answer's `source` on stdout; `--shell` is the one thing a hook has to say,
and `--session-id` defaults to a random UUID. The hooks in `shell/` are the
whole caller:

```sh
command-not-found-agent run --shell bash -- cowsay hi
```

`system-prompt` prints exactly what `run` sends to pi — the base prompts,
the adapter's session and notes context and the generated schemas — which
is what makes prompt changes testable by hand.

`config` prints the settings that came out of every layer, as JSON — the
way to see what a shell will actually use.

`session-id` prints a fresh UUID for a shell to export once at startup:

```sh
export PI_COMMAND_NOT_FOUND_SESSION_ID="$(command-not-found-agent session-id)"
```

### Settings

Everything else is a setting rather than an argument of the call: it is
defined once and read from one place, the config file or the
`PI_COMMAND_NOT_FOUND_*` variable of the same name.

| Setting (config key) | Variable                                  | Default                                                 |
| -------------------- | ----------------------------------------- | ------------------------------------------------------- |
| `pi`                 | `PI_COMMAND_NOT_FOUND_PI`                 | `pi`                                                    |
| `model`              | `PI_COMMAND_NOT_FOUND_MODEL`              | pi default                                              |
| `thinking`           | `PI_COMMAND_NOT_FOUND_THINKING`           | pi default                                              |
| `pi-args`            | `PI_COMMAND_NOT_FOUND_PI_ARGS`            | –                                                       |
| `session-root`       | `PI_COMMAND_NOT_FOUND_SESSION_ROOT`       | `$XDG_STATE_HOME/pi-command-not-found-adapter/sessions` |
| `system-prompt-file` | `PI_COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE` | built-in `base.md`, the Nix prompt in the modules       |
| `mcat`               | `PI_COMMAND_NOT_FOUND_MCAT`               | `mcat`                                                  |
| `width`              | `PI_COMMAND_NOT_FOUND_WIDTH`              | terminal width                                          |
| `retries`            | `PI_COMMAND_NOT_FOUND_RETRIES`            | `2`                                                     |
| `tool-lines`         | `PI_COMMAND_NOT_FOUND_TOOL_LINES`         | `5`                                                     |
| `timeout`            | `PI_COMMAND_NOT_FOUND_TIMEOUT`            | `600`                                                   |
| `trace`              | `PI_COMMAND_NOT_FOUND_TRACE`              | –                                                       |

`pi` and `mcat` are runtime dependencies taken from `PATH` unless set; the
package deliberately does not pin them.

Two settings are lists: `pi-args` holds one argument per item (separated by
newlines in its variable) and `system-prompt-file` one prompt file per item
(separated by colons, like `PATH`).

### Config file

The settings also come from a JSON file, which is how a module or a user
configures the adapter without the shell carrying anything:
`$XDG_CONFIG_HOME/pi-command-not-found/config.json` (the user's, written by
home-manager or by hand) and `/etc/xdg/pi-command-not-found/config.json`
(the system's, written by the NixOS module). They are merged with the user
file last, `--config <FILE>` merges one more over them, and
`PI_COMMAND_NOT_FOUND_CONFIG` replaces the search with the single file it
names. Keys are the setting names above, lists are lists, and an unknown
key is an error:

```json
{
  "model": "deepseek/deepseek-v4-flash",
  "thinking": "medium",
  "pi-args": ["--no-color"],
  "system-prompt-file": ["/path/to/prompts/nix.md"],
  "retries": 2
}
```

So the order is the built-in defaults, the system file, the user file, the
`--config` file, the `PI_COMMAND_NOT_FOUND_*` variables, and last the `run`
arguments.

## Shell integration

`shell/` holds the hook to source from your shell config: it exports the
session id once, then calls `run` and sources the answer in the
interactive shell.

| Shell   | File               | Hook                                  |
| ------- | ------------------ | ------------------------------------- |
| bash    | `shell/bash.sh`    | `command_not_found_handle`            |
| zsh     | `shell/zsh.zsh`    | `command_not_found_handler`           |
| fish    | `shell/fish.fish`  | `fish_command_not_found`              |
| nushell | `shell/nushell.nu` | `$env.config.hooks.command_not_found` |

Two shell limits are worth knowing: fish wires the hook's stdout to stderr,
so a command run from there cannot be piped or redirected; nushell's hook
receives only the command name (the full line comes from history) and
cannot change the caller's environment, so the answer runs in a child
`nu`. Bash and zsh run the handler in a subshell, so a `cd`, an `export`
or an alias in the answer does not persist there (fish does apply them in
the current shell).

## Installed files

The Nix package drops the hooks in the standard places:

| Path                                                                              | Used by                                             |
| --------------------------------------------------------------------------------- | --------------------------------------------------- |
| `bin/command-not-found-agent`                                                     | `PATH`                                              |
| `share/pi-command-not-found-adapter/shell/{bash.sh,zsh.zsh,fish.fish,nushell.nu}` | explicit `source`                                   |
| `share/fish/vendor_conf.d/pi-command-not-found-adapter.fish`                      | fish, from `XDG_DATA_DIRS`                          |
| `share/nushell/vendor/autoload/pi-command-not-found-adapter.nu`                   | nushell's interactive session, from `XDG_DATA_DIRS` |
| `etc/profile.d/pi-command-not-found-adapter.sh`                                   | bash and zsh login shells                           |
| `share/pi-command-not-found-adapter/prompts/{base,nix}.md`                        | the `system-prompt-file` setting                    |

`passthru.shell.{bash,zsh,fish,nushell}` names the explicit-sourcing paths
and `passthru.prompts.{base,nix}` the prompt files, for a NixOS or
home-manager config:

```nix
programs.bash.interactiveShellInit = ''
  source ${pkgs.pi-command-not-found-adapter.passthru.shell.bash}
'';
```

## System prompts

The built-in prompt (`prompts/base.md`) is generic; `prompts/nix.md` is a
full replacement written for a NixOS machine — it teaches the agent to find
an attribute with `nix-locate`, to check it with `nix eval` and to answer
with `nix shell nixpkgs#<attr> -c …`. The `system-prompt-file` setting takes
several files and concatenates them in order, so prompts can be composed;
one file replaces the built-in base, and listing `passthru.prompts.base`
first keeps the generic rules. The NixOS and home-manager modules default
`systemPromptFile` to the Nix prompt, so a machine the package manages gets
it without configuration:

```sh
PI_COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE=…/prompts/nix.md:~/.config/command-not-found/local.md \
  command-not-found-agent run --shell bash -- cowsay hi
```

On a light terminal the note is rendered with mcat's light theme: the
adapter asks the terminal for its background (OSC 11), and passes
`--theme makurai-light` when it is light. When `MCAT_THEME` is already set
the terminal is not queried at all, and an unqueryable terminal just gets
mcat's default theme.

## Answer protocol

The user message is JSON:

```json
{
  "session_id": "…",
  "shell": "bash",
  "cwd": "/home/user",
  "input": "cowsay hi",
  "session_file": "…/sessions/…/session.jsonl",
  "state_dir": "…/pi-command-not-found-adapter"
}
```

The agent answers with exactly one JSON object; either field may be
omitted:

```json
{ "markdown": "short note for the user", "source": "shell code" }
```

`source` is printed on stdout, unchanged, for the caller to source; the
note, the `⚡` announcement and the progress block all go to stderr.

Both schemas are generated from the Rust types with `schemars` — the doc
comments become the field descriptions — and appended to the system
prompt, so the prompt cannot drift from the parser. When the answer does
not parse, the adapter asks again **in the same session**
(`retries` times) instead of starting over.

## System prompt

The prompt is assembled from three parts:

1. the base prompts — `prompts/base.md` by default, or the files given to
   the `system-prompt-file` setting concatenated in order;
2. the adapter-owned context — `prompts/history.md`: what the session file
   and the history directory are, and how to keep notes;
3. the generated input and answer schemas.

Only part 1 is replaceable; 2 and 3 must stay in sync with the adapter.
Nothing that varies per invocation is in here: the session file and state
paths travel in the input instead, so the system prompt is byte-identical
across runs and `system-prompt` can be diffed against a change.

## Logging

Per session: `$XDG_STATE_HOME/pi-command-not-found-adapter/sessions/<session_id>/`
(`session.jsonl`, pi's own session, resumed on every invocation). Per
invocation: `history/<UTC time>/{input,markdown,source}`. Directories are
`0700`, files `0600`.

Diagnostics go through `tracing` to stderr — a warning when no session id
was supplied, debug detail for the pi command and the history directory.
`RUST_LOG` selects the level (default `warn`).

## Layout

| Module        | Role                                            |
| ------------- | ----------------------------------------------- |
| `main.rs`     | subcommand dispatch, wiring and exit status     |
| `cli.rs`      | clap commands, flags and env                    |
| `config.rs`   | layered config files, filled into unset options |
| `session.rs`  | session id, paths, history files                |
| `protocol.rs` | `Input`/`Answer` types, schema, answer parsing  |
| `prompt.rs`   | prompt assembly and the retry message           |
| `pi.rs`       | `pi --mode rpc` process, event decoding         |
| `ask.rs`      | turn loop, retry loop, tool summaries           |
| `ui.rs`       | rolling progress block, spinner, clipping       |
| `markdown.rs` | mcat rendering with a plain-text fallback       |
| `signals.rs`  | SIGINT remembered, TERM/HUP exit                |

See [docs/DESIGN.md](docs/DESIGN.md) for the crate selection and design
rationale.

## Development

The root flake only packages the adapter and its modules; the development
environment, the formatter and the checks live in `nix/develop`, which is
also what `.envrc` loads — `direnv allow` once is enough:

```sh
nix develop ./nix/develop -c cargo test
nix develop ./nix/develop -c cargo clippy --all-targets
treefmt                        # in the dev shell: nixfmt, rustfmt, prettier, shellcheck, actionlint
nix build .#                   # builds the package and runs the tests in the sandbox
nix flake check ./nix/develop  # package, config-file, hooks, formatting
dev/pi.sh                      # interactive pi with the handler's prompt and session
```

Two of those checks guard what the modules hand over: `config-file` feeds a
module-generated config to the binary and `hooks` parses each shell hook in
its own shell. CI (`.github/workflows/check.yml`) runs every check from the
matrix `nix/develop` generates.

`dev/pi.sh` enters an interactive pi configured like the handler, so a
prompt change can be tried by hand: it reuses the same system prompt,
session and notes directory, and accepts pi's own options. Set
`PI_COMMAND_NOT_FOUND_SESSION_ID` for another session and
`PI_COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE` for other base prompts.
