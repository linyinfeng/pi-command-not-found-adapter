#!/bin/sh
# Enter an interactive pi with the same system prompt, session and notes as
# the command-not-found handler, for trying prompt changes by hand.
#
#   dev/pi.sh [pi options...]
#
# Environment:
#   PI_COMMAND_NOT_FOUND_AGENT                 adapter binary (default: nix build)
#   PI_COMMAND_NOT_FOUND_SESSION_ID            session to enter (default: test)
#   PI_COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE    extra base prompts, colon separated
#   PI_COMMAND_NOT_FOUND_MODEL                 model for pi
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
agent=${PI_COMMAND_NOT_FOUND_AGENT:-"$(nix build "$root" --no-link --print-out-paths)/bin/command-not-found-agent"}
session_id=${PI_COMMAND_NOT_FOUND_SESSION_ID:-test}
state=${XDG_STATE_HOME:-"$HOME/.local/state"}/pi-command-not-found-adapter

command -v pi >/dev/null || {
  echo "pi is not in PATH" >&2
  exit 1
}

prompt=$("$agent" system-prompt)
mkdir -p "$state/sessions/$session_id"
if [ -n "${PI_COMMAND_NOT_FOUND_MODEL:-}" ]; then
  set -- --model "$PI_COMMAND_NOT_FOUND_MODEL" "$@"
fi

exec pi \
  --no-context-files \
  --append-system-prompt "$prompt" \
  --session "$state/sessions/$session_id/session.jsonl" \
  "$@"
