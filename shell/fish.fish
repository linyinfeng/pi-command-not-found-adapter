# fish: source this from ~/.config/fish/config.fish

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
