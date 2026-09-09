# pi-command-not-found-adapter

A shell `command-not-found` handler that asks
[pi](https://github.com/earendil-works/pi-mono) for a command, shows the
agent's work, and runs the answer.

```
$ sl
🔧bash: command -v sl
🔧bash: nix eval --raw nixpkgs#sl.name
`sl` isn't installed; it is in nixpkgs.
⚡ nix shell nixpkgs#sl -c sl
────────────────────────────────────────────────────────────────────────
```

The binary is the whole agent: it spawns `pi --mode rpc`, streams one
conversation per shell session, renders the progress block itself, and runs
the resulting command with `bash -c`, forwarding its exit status.

## Usage

```sh
command-not-found-agent [OPTIONS] <command> [args...]
```

Everything after the first positional argument is the command line the user
typed, so put options first:

```sh
command-not-found-agent --model anthropic/claude-haiku-4-5 cowsay hi
```

| Option | Environment | Default |
| --- | --- | --- |
| `--pi <PATH>` | `COMMAND_NOT_FOUND_PI` | `pi` |
| `--model <MODEL>` | `COMMAND_NOT_FOUND_MODEL` | pi default |
| `--thinking <LEVEL>` | `COMMAND_NOT_FOUND_THINKING` | pi default |
| `--pi-arg <ARG>` | `COMMAND_NOT_FOUND_PI_ARGS` | – |
| `--session-id <ID>` | `COMMAND_NOT_FOUND_SESSION_ID` | random UUID |
| `--session-root <DIR>` | `COMMAND_NOT_FOUND_SESSION_ROOT` | `~/.pi/command-not-found/sessions` |
| `--system-prompt-file <FILE>` | `COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE` | built-in |
| `--mdcat <PATH>` | `COMMAND_NOT_FOUND_MDCAT` | `mdcat` |
| `--width <COLUMNS>` | `COMMAND_NOT_FOUND_WIDTH` | terminal width |
| `--retries <N>` | `COMMAND_NOT_FOUND_RETRIES` | `2` |
| `--tool-lines <N>` | `COMMAND_NOT_FOUND_TOOL_LINES` | `5` |
| `--timeout <SECONDS>` | `COMMAND_NOT_FOUND_TIMEOUT` | `600` |
| `--trace <FILE>` | `COMMAND_NOT_FOUND_TRACE` | – |
| `--dry-run` | – | off |

## Answer protocol

The user message is JSON:

```json
{"session_id": "…", "cwd": "/home/user", "input": "cowsay hi"}
```

The agent must answer with exactly one JSON object:

```json
{"markdown": "short note for the user", "command": "shell command"}
```

Both schemas are generated from the Rust types with `schemars` and appended
to the system prompt, so the prompt cannot drift from the parser. When the
answer does not parse, the adapter asks again **in the same session**
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
`history/<UTC time>/{input,markdown,command,status,stdout,stderr}`; the
command's output is tee'd to the terminal while it runs. Directories are
`0700`, files `0600`, `status` stays empty if the run was interrupted.

## Layout

| Module | Role |
| --- | --- |
| `main.rs` | wiring and exit status |
| `cli.rs` | clap configuration (flags + env) |
| `session.rs` | session id, paths, history files |
| `protocol.rs` | `Input`/`Answer` types, schema, answer parsing |
| `prompt.rs` | prompt assembly and the retry message |
| `pi.rs` | `pi --mode rpc` process, event decoding |
| `ask.rs` | turn loop, retry loop, tool summaries |
| `ui.rs` | rolling progress block, spinner, clipping |
| `markdown.rs` | mdcat rendering with a plain-text fallback |
| `exec.rs` | `bash -c`, output tee, exit status |
| `signals.rs` | SIGINT counting, TERM/HUP exit |

See [docs/DESIGN.md](docs/DESIGN.md) for the crate selection and design
rationale.

## Development

```sh
nix-shell --run 'cargo test'
nix-shell --run 'cargo clippy --all-targets'
```
