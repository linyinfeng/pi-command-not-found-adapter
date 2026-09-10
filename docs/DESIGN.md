# Design

## One binary

The shell handler, the pi driver, the progress UI and the log writer are
one process. That keeps the conversation state in one place and means the
only runtime dependencies are `pi` and (for pretty notes) `mcat` — both
from `PATH` and both overridable. Nothing is executed by the adapter: the
answer goes back to the shell that asked.

## Driving pi

Two ways to talk to pi:

| | `--mode json` | `--mode rpc` |
| --- | --- | --- |
| direction | one shot, stdout events | request/response on stdin + stdout events |
| multi-turn | new process per turn (`--session`) | one process, repeated `prompt` commands |
| turn boundary | process exit | `agent_settled` event |
| dialogs | none | `extension_ui_request` needs an answer |

`--mode rpc` is used because the retry requirement — "if the answer does
not parse, ask again in the same session" — is a multi-turn loop. One
process keeps the session, the model and the tool state warm, and
`agent_settled` is an explicit "this run is over" signal instead of a
process exit. Dialog requests (`select`, `confirm`, `input`, `editor`) are
answered with `cancelled: true` so an extension can never block the
handler; fire-and-forget requests (`setStatus`, `setWidget`, …) are
ignored.

## Crate selection

Versions and download figures from crates.io, September 2026.

| Need | Choice | Version | Why |
| --- | --- | --- | --- |
| CLI + env | `clap` (derive, env) | 4.6 | `#[arg(long, env = …)]` gives every option an environment variable; the standard choice |
| errors | `anyhow` | 1.0 | one error type for a binary; context added at each boundary |
| logs | `tracing` + `tracing-subscriber` | 0.1 / 0.3 | warnings and debug diagnostics on stderr, `RUST_LOG` filter |
| JSON | `serde` + `serde_json` | 1.0 | `Input`/`Answer` and pi's events |
| XDG dirs | `dirs` | 7.0 | `state_dir()` gives `$XDG_STATE_HOME` (or `~/.local/state`) for the session and note storage |
| schema | `schemars` | 1.2 | derives JSON Schema from the same structs that deserialize the answer, so prompt and parser cannot drift |
| terminal | `console` | 0.16 | one crate for TTY detection, terminal size, `move_cursor_up`, `clear_last_lines`, `clear_line`, styling, and East-Asian-aware `measure_text_width` |
| terminal colours | `terminal-colorsaurus` | 1.0 | OSC 10/11 background query with a DA1 ordering heuristic, a raw-mode guard and a timeout; reuses stdio or `/dev/tty` |
| signals | `signal-hook` | 0.4 | a signal iterator on its own thread; SIGINT counting plus TERM/HUP exit |
| processes | `std::process` + `std::thread` | – | no async runtime needed |

Rejected alternatives:

- **`indicatif`** (0.18): built for progress bars with its own draw target
  and refresh thread. The block here is six lines with a rolling window, a
  cloud counter and wide-character clipping; the `console` primitives are
  enough, and it avoids a second layer that owns stderr. If the block grows
  into many independent bars, `indicatif`'s `MultiProgress` is the upgrade
  path.
- **`crossterm`** (0.29): the right crate for raw-mode/event-reading TUIs.
  This UI is write-only, and `console` already provides cursor movement,
  clearing, size and text measurement, so `crossterm` would be a second
  terminal dependency for the same escapes.
- **`ratatui`** (0.30): full-screen widget framework; wrong model for
  inline scrollback output.
- **`tokio`**: two blocking pipes (pi stdout and stdin) map directly onto
  threads; an async runtime would add complexity, not throughput.
- **`ctrlc`** (3.5): SIGINT-focused; `signal-hook` covers TERM/HUP and
  gives a clean iterator.
- **`time`/`jiff`**: the only timestamp needed is the history directory
  name; the civil-date conversion is fifteen lines and covered by a test.
- **`directories`/`etcetera`**: `directories` wraps the same `dirs-sys`
  calls with project-directory helpers we do not need (one `join` does
  it), and its last release is older than `dirs`' 7.0.
- **`uuid`**: session ids fall back to `/proc/sys/kernel/random/uuid`, the
  same source the shell init uses.

## Markdown rendering

`mcat` runs as a subprocess, invoked as `mcat -P --silent --sc <width>xauto`
plus `-c` or `-C`: `mcat` drops ANSI when its own stdout is not a terminal,
and ours is a pipe on the way to the user's terminal. `-P` keeps it from
paging, `--silent` drops the loading bars.

`mcat` was picked over `mdcat` for images: it renders them inline through
kitty/iTerm2/sixel and reports terminal capabilities, which the `mdcat`
package does not. The price is style — `mcat` uses truecolor and background
colors and is themed (`MCAT_THEME`, `-t/--theme`) — and the CLI is the
contract: `--mcat` points at the binary, so a wrapper can adjust the theme.
A missing or failing renderer falls back to the raw text.

`termimad` (in-process, `crossterm`-based) was the runner-up: no runtime
dependency, but a different look and a styling layer for a note of a few
lines.

### Light and dark

mcat ships one light theme (`makurai-light`); every other theme it has is
dark, and the default (`github`) is dark. So the note only needs a hint
when the terminal is light, and `terminal-colorsaurus` supplies it: it
sends `OSC 11` plus `DA1`, and because terminals answer in order, a `DA1`
reply arriving first proves the colour query is unsupported — no timeout
needed for that verdict. It writes to and reads from the tty itself
(stderr or `/dev/tty`), which matters here because our stdout is a pipe
for the caller to source.

The query runs once, right before the note is rendered, and never when
`MCAT_THEME` is set. Measured on a responding terminal: 0.2–0.4 ms; on all
redirected streams it fails in microseconds; a terminal that answers
neither query costs the 300 ms timeout once and then gets the default
theme. It is deliberately synchronous: the crate holds the terminal in raw
mode while waiting, and a background thread could be killed by a signal or
by exiting before its guard restores the terminal.

The alternatives were rejected for our pipe-heavy situation:
`terminal-light` writes the query to stdout and reads stdin (both are a
pipe here), `termbg` hardcodes stdio and pulls in an async runtime, and
`dark-light` asks the desktop environment rather than the terminal.

## Behaviour

- **Sessions.** One conversation per shell: the shell init exports
  `COMMAND_NOT_FOUND_SESSION_ID` (which `command-not-found-agent session-id`
  can produce), the adapter keeps pi's session at
  `$XDG_STATE_HOME/pi-command-not-found-adapter/sessions/<session_id>/session.jsonl`
  and resumes it on every invocation, so a thread continues across
  commands. Without an id it warns and reads a UUID from
  `/proc/sys/kernel/random/uuid`. The state directory also holds the
  agent's notes; both paths travel in the input, so the system prompt is
  identical for every invocation.
- **Answers.** The model must reply with one JSON object; the schema is
  generated from `protocol::Answer` and appended to the system prompt, so
  the field descriptions live with the types. Both fields are optional:
  omit `source` when nothing should run. `parse_answer` scans the
  assistant text for JSON objects and keeps the last one that carries at
  least one of the two fields. Anything else — an unrelated object, a
  mistyped field — is not an answer, and the adapter asks again in the
  same session (`--retries`, default 2) instead of starting a new
  conversation.
- **Sourced answers.** `source` is printed on stdout unchanged; the caller
  sources it, so it runs in the user's interactive shell with the user's
  environment and can `cd`, export or define things — and the shell name
  travels in the input (`--shell`) so the model can write that syntax.
  The shell is never detected; the caller knows it and passes it. bash and
  zsh invoke the handler in a subshell, so state changes from an answer do
  not persist there; fish applies them in the current shell.
- **Progress.** A braille spinner ticks at 10 Hz while the turn runs, `💭`
  accumulates on each thinking block, and tool calls appear as clipped
  `🔧name: argument` lines in a rolling block of `--tool-lines` (default
  5) that is redrawn in place and erased before the note is printed.
- **Exit status.** 0 once an answer was produced (even an empty one), 1
  when pi failed or the answer never parsed, 130 on Ctrl-C — an interrupt
  anywhere in the turn stops it, and one that arrives while the note is
  rendering still wins, so the shell is never handed code after a Ctrl-C.
- **Logging.** `history/<UTC time>/{input,markdown,source}` next to the
  session, `0600` files in `0700` directories.
