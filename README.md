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
  programs.pi-command-not-found-adapter.enable = true;
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

The NixOS module sources the hook for bash and zsh (gated on
`programs.bash.enable` / `programs.zsh.enable`); fish and nushell need no
config there, they auto-load the hook from the package's data directories
on `XDG_DATA_DIRS`. The home-manager module sources all four shells
through their `programs.*` options, because home-manager keeps package
data directories off `XDG_DATA_DIRS`. `programs.pi-command-not-found-adapter.package`
defaults to `pkgs.pi-command-not-found-adapter` (provided by the overlay
above); override it to use a different build.

## CLI

```sh
command-not-found-agent run [OPTIONS] -- <command> [args...]
command-not-found-agent session-id
command-not-found-agent system-prompt [OPTIONS]
```

`run` is the handler: it takes the command line after `--` (options come
before it) and prints the answer's `source` on stdout. The shell hooks in
`shell/` call it, for example:

```sh
command-not-found-agent run --shell bash --model deepseek/deepseek-v4-flash -- cowsay hi
```

`system-prompt` prints exactly what `run` sends to pi — the base prompts,
the adapter's session and notes context and the generated schemas — which
is what makes prompt changes testable by hand.

`session-id` prints a fresh UUID for a shell to export once at startup:

```sh
export COMMAND_NOT_FOUND_SESSION_ID="$(command-not-found-agent session-id)"
```

### `run` options

| Option | Environment | Default |
| --- | --- | --- |
| `--pi <PATH>` | `COMMAND_NOT_FOUND_PI` | `pi` |
| `--model <MODEL>` | `COMMAND_NOT_FOUND_MODEL` | pi default |
| `--thinking <LEVEL>` | `COMMAND_NOT_FOUND_THINKING` | pi default |
| `--pi-arg <ARG>` | `COMMAND_NOT_FOUND_PI_ARGS` | – |
| `--session-id <ID>` | `COMMAND_NOT_FOUND_SESSION_ID` | random UUID |
| `--shell <SHELL>` | `COMMAND_NOT_FOUND_SHELL` | required |
| `--session-root <DIR>` | `COMMAND_NOT_FOUND_SESSION_ROOT` | `$XDG_STATE_HOME/pi-command-not-found-adapter/sessions` |
| `--system-prompt-file <FILE>` | `COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE` | built-in `base.md` |
| `--mcat <PATH>` | `COMMAND_NOT_FOUND_MCAT` | `mcat` |
| `--width <COLUMNS>` | `COMMAND_NOT_FOUND_WIDTH` | terminal width |
| `--retries <N>` | `COMMAND_NOT_FOUND_RETRIES` | `2` |
| `--tool-lines <N>` | `COMMAND_NOT_FOUND_TOOL_LINES` | `5` |
| `--timeout <SECONDS>` | `COMMAND_NOT_FOUND_TIMEOUT` | `600` |
| `--trace <FILE>` | `COMMAND_NOT_FOUND_TRACE` | – |

## Shell integration

`shell/` holds the hook to source from your shell config: it exports the
session id once, then calls `run` and sources the answer in the
interactive shell.

| Shell | File | Hook |
| --- | --- | --- |
| bash | `shell/bash.sh` | `command_not_found_handle` |
| zsh | `shell/zsh.zsh` | `command_not_found_handler` |
| fish | `shell/fish.fish` | `fish_command_not_found` |
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

| Path | Used by |
| --- | --- |
| `bin/command-not-found-agent` | `PATH` |
| `share/pi-command-not-found-adapter/shell/{bash.sh,zsh.zsh,fish.fish,nushell.nu}` | explicit `source` |
| `share/fish/vendor_conf.d/pi-command-not-found-adapter.fish` | fish, from `XDG_DATA_DIRS` |
| `share/nushell/vendor/autoload/pi-command-not-found-adapter.nu` | nushell's interactive session, from `XDG_DATA_DIRS` |
| `etc/profile.d/pi-command-not-found-adapter.sh` | bash and zsh login shells |
| `share/pi-command-not-found-adapter/prompts/{base,nix}.md` | the `system-prompt-file` setting |

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
first keeps the generic rules:

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
{"markdown": "short note for the user", "source": "shell code"}
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

| Module | Role |
| --- | --- |
| `main.rs` | subcommand dispatch, wiring and exit status |
| `cli.rs` | clap commands, flags and env |
| `config.rs` | layered config files, filled into unset options |
| `session.rs` | session id, paths, history files |
| `protocol.rs` | `Input`/`Answer` types, schema, answer parsing |
| `prompt.rs` | prompt assembly and the retry message |
| `pi.rs` | `pi --mode rpc` process, event decoding |
| `ask.rs` | turn loop, retry loop, tool summaries |
| `ui.rs` | rolling progress block, spinner, clipping |
| `markdown.rs` | mcat rendering with a plain-text fallback |
| `signals.rs` | SIGINT remembered, TERM/HUP exit |

See [docs/DESIGN.md](docs/DESIGN.md) for the crate selection and design
rationale.

## Development

```sh
nix develop -c cargo test
nix develop -c cargo clippy --all-targets
nix build .#          # builds the package and runs the tests in the sandbox
dev/pi.sh             # interactive pi with the handler's prompt and session
```

`dev/pi.sh` enters an interactive pi configured like the handler, so a
prompt change can be tried by hand: it reuses the same system prompt,
session and notes directory, and accepts pi's own options. Set
`PI_COMMAND_NOT_FOUND_SESSION_ID` for another session and
`PI_COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE` for other base prompts.
