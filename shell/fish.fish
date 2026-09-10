# fish: source this from ~/.config/fish/config.fish
#
# fish runs this function with its stdout wired to stderr, so a command run
# from here cannot be piped or redirected (`cowsay hi | cat` loses the
# output); bash and zsh do not have this limitation.

set -q COMMAND_NOT_FOUND_SESSION_ID
and test -n "$COMMAND_NOT_FOUND_SESSION_ID"
or set -gx COMMAND_NOT_FOUND_SESSION_ID (command-not-found-agent session-id)

function fish_command_not_found
    set -l code (command-not-found-agent run --shell fish -- $argv | string collect)
    if test $pipestatus[1] -ne 0
        return $pipestatus[1]
    end
    eval $code
end
