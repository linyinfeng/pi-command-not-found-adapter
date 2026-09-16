# The defaults the Nix modules share. They live in `config` rather than in the
# option declarations because both depend on the `package` option, and an
# option default may not read other options; `mkDefault` gives them the lowest
# priority, so a plain assignment in a user's configuration still wins.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.pi-command-not-found-adapter;
in
{
  # Behind `enable`: without it, importing the module must not force the
  # package option (which needs the overlay) just to read its prompt paths.
  config = lib.mkIf cfg.enable {
    # On a machine this package manages, the Nix prompt is the right one.
    programs.pi-command-not-found-adapter.systemPromptFile = lib.mkDefault [
      cfg.package.passthru.prompts.nix
    ];

    # GNU mtools ships its own `mcat`, which would shadow this one in PATH;
    # the module knows the right package, so point at it (still overridable).
    programs.pi-command-not-found-adapter.mcat = lib.mkDefault (lib.getExe pkgs.mcat);

    # Ours is the hook: the stock nixpkgs one would answer for the same shells
    # when a database exists, and home-manager's nix-index module adds its own
    # per shell, so whichever was sourced last would win.
    programs.command-not-found.enable = lib.mkDefault false;
  };
}
