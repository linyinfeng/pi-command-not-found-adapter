# One entry per setting the adapter reads, written to the JSON config file it
# finds on XDG_CONFIG_DIRS or XDG_CONFIG_HOME; the adapter's own key names and
# variable names are derived from the option name, so nothing is spelled twice.
# Types mirror `Config` in src/config.rs, and a null option leaves the
# adapter's default alone.
{ lib }:
let
  inherit (lib) types;

  spec = {
    pi = {
      type = types.str;
      description = "pi executable to spawn (default `pi`).";
    };
    model = {
      type = types.str;
      example = "deepseek/deepseek-v4-flash";
      description = "Model the adapter asks (default: pi's own).";
    };
    thinking = {
      type = types.str;
      description = "pi thinking level (default: pi's own).";
    };
    piArgs = {
      type = types.listOf types.str;
      join = "\n";
      description = "Extra pi arguments, one per line.";
    };
    sessionRoot = {
      type = types.path;
      description = "Directory holding the per-session state (default: `$XDG_STATE_HOME/pi-command-not-found-adapter/sessions`).";
    };
    systemPromptFile = {
      type = types.listOf types.path;
      join = ":";
      description = ''
        Base prompts to send, joined with colons like `PATH`. The first one
        replaces the built-in `prompts/base.md`, so list
        `passthru.prompts.base` first to keep the generic rules. Both
        modules default this to the Nix prompt, `prompts/nix.md`.
      '';
    };
    mcat = {
      type = types.str;
      description = "mcat executable used to render the note (default `mcat`).";
    };
    width = {
      type = types.ints.positive;
      description = "Width to render at (default: the terminal's width).";
    };
    retries = {
      type = types.ints.unsigned;
      description = "Extra asks after an answer that does not parse (default 2).";
    };
    toolLines = {
      type = types.ints.positive;
      description = "Tool-call lines kept in the progress block (default 5).";
    };
    timeout = {
      type = types.ints.positive;
      description = "Seconds one turn may take (default 600).";
    };
    trace = {
      type = types.path;
      description = "File pi's raw event stream is appended to (default: off).";
    };
  };

  set = cfg: lib.filterAttrs (name: _: cfg.${name} != null) spec;

  # The adapter reads the settings from its own JSON config file, so the
  # values stay data: keys are the option names in kebab case (`pi-args`,
  # `system-prompt-files`, …), lists stay lists and nothing is ever turned
  # into shell code.
  kebab =
    name: lib.toLower (lib.replaceStrings lib.upperChars (map (c: "-" + c) lib.upperChars) name);
  json =
    value:
    if lib.isList value then
      map json value
    else if lib.isInt value then
      value
    else
      toString value;
  settings =
    cfg: lib.mapAttrs' (name: _: lib.nameValuePair (kebab name) (json cfg.${name})) (set cfg);
in
{
  # Spread into the module's `options`.
  options = lib.mapAttrs (
    _: s:
    lib.mkOption {
      type = types.nullOr s.type;
      default = null;
      inherit (s) description;
      example = s.example or null;
    }
  ) spec;

  configFile = pkgs: cfg: pkgs.writeText "pi-command-not-found.json" (builtins.toJSON (settings cfg));
}
