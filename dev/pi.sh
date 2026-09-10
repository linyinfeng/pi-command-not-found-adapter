#!/bin/sh
# Enter an interactive pi with the same system prompt, session and notes as
# the command-not-found handler, for trying prompt changes by hand.
#
#   dev/pi.sh [pi options...]
#
# Environment:
#   COMMAND_NOT_FOUND_AGENT                 adapter binary (default: nix build)
#   COMMAND_NOT_FOUND_SESSION_ID            session to enter (default: test)
#   COMMAND_NOT_FOUND_SYSTEM_PROMPT_FILE    extra base prompts, colon separated
#   COMMAND_NOT_FOUND_MODEL                 model for pi
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
agent=${COMMAND_NOT_FOUND_AGENT:-"$(nix build "$root" --no-link --print-out-paths)/bin/command-not-found-agent"}
session_id=${COMMAND_NOT_FOUND_SESSION_ID:-test}
state=${XDG_STATE_HOME:-"$HOME/.local/state"}/pi-command-not-found-adapter

command -v pi >/dev/null || {
  echo "pi is not in PATH" >&2
  exit 1
}

prompt=$("$agent" system-prompt)
mkdir -p "$state/sessions/$session_id"
if [ -n "${COMMAND_NOT_FOUND_MODEL:-}" ]; then
  set -- --model "$COMMAND_NOT_FOUND_MODEL" "$@"
fi

exec pi \
  --no-context-files \
  --append-system-prompt "$prompt" \
  --session "$state/sessions/$session_id/session.jsonl" \
  "$@"
