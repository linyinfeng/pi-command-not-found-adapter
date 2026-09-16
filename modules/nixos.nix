{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.pi-command-not-found-adapter;
  vars = import ./options.nix { inherit lib; };
  configFile = vars.configFile pkgs cfg;
  inherit (lib) mkIf mkEnableOption mkPackageOption;
in
{
  options.programs.pi-command-not-found-adapter = {
    enable = mkEnableOption "the pi command-not-found adapter";
    package = mkPackageOption pkgs "pi-command-not-found-adapter" { };
  }
  // vars.options;

  config = mkIf cfg.enable {
    # The settings are a JSON file the adapter finds on XDG_CONFIG_DIRS, so
    # nothing is exported: each shell only sources the hook, and fish and
    # nushell may also auto-load it from the package's data directories
    # (idempotent either way). A user's own ~/.config file layers on top.
    environment.systemPackages = [ cfg.package ];
    environment.etc."xdg/pi-command-not-found/config.json".source = configFile;

    # This module is the handler: the stock nixpkgs hook would otherwise
    # answer for the same shells (its own default is on when a database
    # exists) and whichever was sourced last would win.
    programs.command-not-found.enable = lib.mkDefault false;

    programs.pi-command-not-found-adapter.systemPromptFile = lib.mkDefault (vars.nixPrompt cfg);

    programs.bash.interactiveShellInit = mkIf config.programs.bash.enable ''
      source ${cfg.package.passthru.shell.bash}
    '';
    programs.zsh.interactiveShellInit = mkIf config.programs.zsh.enable ''
      source ${cfg.package.passthru.shell.zsh}
    '';
    programs.fish.interactiveShellInit = mkIf config.programs.fish.enable ''
      source ${cfg.package.passthru.shell.fish}
    '';
    programs.nushell.autoloads = mkIf config.programs.nushell.enable (
      lib.mkAfter [
        (pkgs.writeTextDir "share/nushell/vendor/autoload/10-command-not-found.nu" ''
          source ${cfg.package.passthru.shell.nushell}
        '')
      ]
    );
  };
}
