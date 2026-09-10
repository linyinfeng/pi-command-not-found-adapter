# zsh: source this from ~/.zshrc

export COMMAND_NOT_FOUND_SESSION_ID="${COMMAND_NOT_FOUND_SESSION_ID:-$(command-not-found-agent session-id)}"

command_not_found_handler() {
  local code
  code=$(command-not-found-agent run --shell zsh -- "$@") || return $?
  eval "$code"
}
