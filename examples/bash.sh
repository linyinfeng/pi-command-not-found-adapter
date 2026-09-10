# bash: source this from ~/.bashrc

export COMMAND_NOT_FOUND_SESSION_ID="${COMMAND_NOT_FOUND_SESSION_ID:-$(command-not-found-agent session-id)}"

command_not_found_handle() {
  local code
  code=$(command-not-found-agent run --shell bash -- "$@") || return $?
  eval "$code"
}
