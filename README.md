# pi-command-not-found-adapter

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
code runs in the user's own shell, not in a subshell.

## Usage

```sh
command-not-found-agent run [OPTIONS] -- <command> [args...]
command-not-found-agent session-id
```

`run` is the handler: it takes the command line after `--` (options come
before it) and prints the answer's `source` on stdout. A shell wraps it
like this:

```sh
command_not_found_handle() {
  local code status
  code=$("$COMMAND_NOT_FOUND_AGENT" run --shell bash -- "$@") || return $?
  eval "$code"
}
```

Standalone:

```sh
command-not-found-agent run --shell bash --model anthropic/claude-haiku-4-5 -- cowsay hi
```

`session-id` prints a fresh UUID for a shell to export once at startup:

```sh
export COMMAND_NOT_FOUND_SESSION_ID="$(command-not-found-agent session-id)"
```

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
`nu`.

### `run` options

| Option | Environment | Default |
| --- | --- | --- |
| `--pi <PATH>` | `COMMAND_NOT_FOUND_PI` | `pi` |
| `--model <MODEL>` | `COMMAND_NOT_FOUND_MODEL` | pi default |
| `--thinking <LEVEL>` | `COMMAND_NOT_FOUND_THINKING` | pi default |
| `--pi-arg <ARG>` | `COMMAND_NOT_FOUND_PI_ARGS` | – |
| `--session-id <ID>` | `COMMAND_NOT_FOUND_SESSION_ID` | random UUID |
| `--shell <SHELL>` | `COMMAND_NOT_FOUND_SHELL` | required |
| `--session-root <DIR>` | `COMMAND_NOT_FOUND_SESSION_ROOT` | `~/.pi/command-not-found/sessions` |
| `--system-prompt-file <FILE>` | `COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE` | built-in |
| `--mdcat <PATH>` | `COMMAND_NOT_FOUND_MDCAT` | `mdcat` |
| `--width <COLUMNS>` | `COMMAND_NOT_FOUND_WIDTH` | terminal width |
| `--retries <N>` | `COMMAND_NOT_FOUND_RETRIES` | `2` |
| `--tool-lines <N>` | `COMMAND_NOT_FOUND_TOOL_LINES` | `5` |
| `--timeout <SECONDS>` | `COMMAND_NOT_FOUND_TIMEOUT` | `600` |
| `--trace <FILE>` | `COMMAND_NOT_FOUND_TRACE` | – |

## Answer protocol

The user message is JSON:

```json
{
  "session_id": "…",
  "shell": "bash",
  "cwd": "/home/user",
  "input": "cowsay hi"
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
(`--retries` times) instead of starting over.

## System prompt

The prompt is assembled from three parts:

1. the base prompt — `prompts/base.md`, replaced by `--system-prompt-file`;
2. the adapter-owned context — `prompts/history.md`: where the session and
   the history directory live, and how to keep notes there;
3. the generated input and answer schemas.

Only part 1 is replaceable; 2 and 3 must stay in sync with the adapter.

## Logging

Per session: `~/.pi/command-not-found/sessions/<session_id>/session.jsonl`
(pi's own session, resumed on every invocation). Per invocation:
`history/<UTC time>/{input,markdown,source}`. Directories are `0700`,
files `0600`.

Diagnostics go through `tracing` to stderr — a warning when no session id
was supplied, debug detail for the pi command and the history directory.
`RUST_LOG` selects the level (default `warn`).

## Layout

| Module | Role |
| --- | --- |
| `main.rs` | subcommand dispatch, wiring and exit status |
| `cli.rs` | clap commands, flags and env |
| `session.rs` | session id, paths, history files |
| `protocol.rs` | `Input`/`Answer` types, schema, answer parsing |
| `prompt.rs` | prompt assembly and the retry message |
| `pi.rs` | `pi --mode rpc` process, event decoding |
| `ask.rs` | turn loop, retry loop, tool summaries |
| `ui.rs` | rolling progress block, spinner, clipping |
| `markdown.rs` | mdcat rendering with a plain-text fallback |
| `signals.rs` | SIGINT counting, TERM/HUP exit |

See [docs/DESIGN.md](docs/DESIGN.md) for the crate selection and design
rationale.

## Development

```sh
nix-shell --run 'cargo test'
nix-shell --run 'cargo clippy --all-targets'
```

> [!NOTE]
> This project was developed with LLM assistance.
