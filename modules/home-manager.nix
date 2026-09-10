{ config, lib, pkgs, ... }:

let
  cfg = config.programs.pi-command-not-found-adapter;
  inherit (lib) mkIf mkEnableOption mkPackageOption;
in
{
  options.programs.pi-command-not-found-adapter = {
    enable = mkEnableOption "the pi command-not-found adapter";
    package = mkPackageOption pkgs "pi-command-not-found-adapter" { };
  };

  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];

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
