# fish: source this from ~/.config/fish/config.fish
#
# fish runs this function with its stdout wired to stderr, so a command run
# from here cannot be piped or redirected (`cowsay hi | cat` loses the
# output); bash and zsh do not have this limitation.

set -q COMMAND_NOT_FOUND_SESSION_ID
and test -n "$COMMAND_NOT_FOUND_SESSION_ID"
or set -gx COMMAND_NOT_FOUND_SESSION_ID (command-not-found-agent session-id)

function fish_command_not_found
    # Without the binary the hook would recurse on itself; report 127 instead.
    command -q command-not-found-agent; or return 127
    set -l code (command-not-found-agent run --shell fish -- $argv | string collect)
    # fish resets $pipestatus on every command, so read the agent's status
    # before the next builtin clobbers it.
    set -l ret $pipestatus[1]
    if test $ret -ne 0
        return $ret
    end
    eval $code
end
