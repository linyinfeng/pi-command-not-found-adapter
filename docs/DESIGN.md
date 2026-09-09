# Design

## One binary

The shell handler, the pi driver, the progress UI, the log writer and the
command runner are one process. That keeps the conversation state in one
place and means the only runtime dependencies are `pi`, `bash` and (for
pretty notes) `mdcat` — all from `PATH` and all overridable.

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
| JSON | `serde` + `serde_json` | 1.0 | `Input`/`Answer` and pi's events |
| schema | `schemars` | 1.2 | derives JSON Schema from the same structs that deserialize the answer, so prompt and parser cannot drift |
| terminal | `console` | 0.16 | one crate for TTY detection, terminal size, `move_cursor_up`, `clear_last_lines`, `clear_line`, styling, and East-Asian-aware `measure_text_width` |
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
- **`tokio`**: three blocking pipes (pi stdout, command stdout/stderr) map
  directly onto threads; an async runtime would add complexity, not
  throughput.
- **`ctrlc`** (3.5): SIGINT-focused; `signal-hook` covers TERM/HUP and
  gives a clean iterator.
- **`time`/`jiff`**: the only timestamp needed is the history directory
  name; the civil-date conversion is fifteen lines and covered by a test.
- **`uuid`**: session ids fall back to `/proc/sys/kernel/random/uuid`, the
  same source the shell init uses.

## Markdown rendering

`mdcat` runs as a subprocess. Its output style is the target (named ANSI
palette, no background colours), and the `mdcat` *library* pulls `syntect`,
image and sixel support while the crate ecosystem around it (`mdcat` vs
`mdcat-ng`) is in flux. Shelling out keeps byte-identical output with zero
dependency weight; `--mdcat` selects another renderer, and a missing or
failing one falls back to the raw text.

`termimad` (in-process, `crossterm`-based) was the runner-up: it would
remove the runtime dependency but change the look and add a styling layer
for a two-field note.

## Behaviour

- **Sessions.** One conversation per shell: the shell init exports
  `COMMAND_NOT_FOUND_SESSION_ID`, the adapter keeps pi's session at
  `~/.pi/command-not-found/sessions/<session_id>/session.jsonl` and
  resumes it on every invocation, so a thread continues across commands.
  Without an id it reads a UUID from `/proc/sys/kernel/random/uuid`.
- **Answers.** The model must reply with one JSON object; the schema is
  generated from `protocol::Answer` and appended to the system prompt.
  `parse_answer` scans the assistant text for JSON objects and keeps the
  last one that deserializes. A missing or mistyped field is an error, and
  the adapter asks again in the same session (`--retries`, default 2)
  instead of starting a new conversation.
- **Progress.** A braille spinner ticks at 10 Hz while the turn runs, `💭`
  accumulates on each thinking block, and tool calls appear as clipped
  `🔧name: argument` lines in a rolling block of `--tool-lines` (default
  5) that is redrawn in place and erased before the note is printed.
- **Exit status.** The command's own status, `128 + signal` when it was
  killed. The first Ctrl-C reaches the child; the second kills it. SIGTERM
  and SIGHUP exit `128 + signal`.
- **Logging.** `history/<UTC time>/{input,markdown,command,status,stdout,stderr}`
  next to the session, `0600` files in `0700` directories. `status` stays
  empty when the run was interrupted. The command's output is tee'd to the
  terminal while it runs, and a grandchild holding the pipes open cannot
  make the adapter wait forever.
