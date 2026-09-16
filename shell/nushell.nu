# nushell: put this in your config.nu
#
# nushell's command_not_found hook only receives the command name and cannot
# change the caller's environment, so the answer is run in a child `nu`:
# plain commands work, `cd`/env effects do not stick.

$env.PI_COMMAND_NOT_FOUND_SESSION_ID = ($env.PI_COMMAND_NOT_FOUND_SESSION_ID? | default (command-not-found-agent session-id))

$env.config.hooks.command_not_found = { |name|
  # Without the binary the hook would recurse on itself; stay quiet instead.
  if (which command-not-found-agent | is-empty) {
    return
  }
  # History yields the raw line only and nushell has no shell-like tokenizer:
  # `split row ' '` is all we have, so quoted arguments and repeated spaces
  # reach the handler split apart. (ponytail: no parser here, the model
  # reassembles from the words.)
  let line = (history | last | get command? | default $name)
  let result = (command-not-found-agent run --shell nu -- ...($line | split row ' ') | complete)
  # `complete` captures stderr, so the note has to be printed explicitly;
  # bash, zsh and fish pass it through.
  print -e $result.stderr
  if $result.exit_code == 0 {
    nu -c $result.stdout
  }
}
