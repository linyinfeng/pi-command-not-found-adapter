# nushell: put this in your config.nu
#
# nushell's command_not_found hook only receives the command name and cannot
# change the caller's environment, so the answer is run in a child `nu`:
# plain commands work, `cd`/env effects do not stick.

$env.COMMAND_NOT_FOUND_SESSION_ID = ($env.COMMAND_NOT_FOUND_SESSION_ID? | default (command-not-found-agent session-id))

$env.config.hooks.command_not_found = { |name|
  let line = (history | last | get command? | default $name)
  let result = (command-not-found-agent run --shell nu -- ...($line | split row ' ') | complete)
  if $result.exit_code == 0 {
    nu -c $result.stdout
  } else {
    print -e $result.stderr
  }
}
