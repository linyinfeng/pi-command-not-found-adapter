# zsh: source this from ~/.zshrc

export COMMAND_NOT_FOUND_SESSION_ID="${COMMAND_NOT_FOUND_SESSION_ID:-$(command-not-found-agent session-id)}"

command_not_found_handler() {
  # Without the binary the hook would recurse on itself; report 127 instead.
  command -v command-not-found-agent >/dev/null 2>&1 || return 127
  local code
  code=$(command-not-found-agent run --shell zsh -- "$@") || return $?
  eval "$code"
}
