# shellcheck shell=bash
# bash: source this from ~/.bashrc

export PI_COMMAND_NOT_FOUND_SESSION_ID="${PI_COMMAND_NOT_FOUND_SESSION_ID:-$(command-not-found-agent session-id)}"

command_not_found_handle() {
  # Without the binary the hook would recurse on itself; report 127 instead.
  command -v command-not-found-agent >/dev/null 2>&1 || return 127
  local code
  code=$(command-not-found-agent run --shell bash -- "$@") || return $?
  eval "$code"
}
