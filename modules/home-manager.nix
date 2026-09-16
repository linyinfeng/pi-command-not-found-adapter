{ config, lib, pkgs, ... }:

let
  cfg = config.programs.pi-command-not-found-adapter;
  vars = import ./options.nix { inherit lib; };
  configFile = vars.configFile pkgs cfg;
  inherit (lib) mkIf mkEnableOption mkPackageOption;
in
{
  options.programs.pi-command-not-found-adapter =
    {
      enable = mkEnableOption "the pi command-not-found adapter";
      package = mkPackageOption pkgs "pi-command-not-found-adapter" { };
    }
    // vars.options;

  config = mkIf cfg.enable {
    # Same shape as the NixOS module: the settings are a JSON file, here in
    # the user's own config directory, and each shell only sources the hook.
    # `PI_COMMAND_NOT_FOUND_CONFIG` or the environment variables still
    # override individual values.
    home.packages = [ cfg.package ];
    xdg.configFile."pi-command-not-found/config.json".source = configFile;

    programs.bash.initExtra = mkIf config.programs.bash.enable ''
      source ${cfg.package.passthru.shell.bash}
    '';
    # initExtra is deprecated upstream; initContent is its replacement.
    programs.zsh.initContent = mkIf config.programs.zsh.enable ''
      source ${cfg.package.passthru.shell.zsh}
    '';
    programs.fish.interactiveShellInit = mkIf config.programs.fish.enable ''
      source ${cfg.package.passthru.shell.fish}
    '';
    programs.nushell.extraConfig = mkIf config.programs.nushell.enable ''
      source ${cfg.package.passthru.shell.nushell}
    '';
  };
}
