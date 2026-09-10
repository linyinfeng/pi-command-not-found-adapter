{ config, lib, pkgs, ... }:

let
  cfg = config.programs.pi-command-not-found-adapter;
  inherit (lib) mkIf mkEnableOption mkPackageOption;
in
{
  options.programs.pi-command-not-found-adapter = {
    enable = mkEnableOption "the pi command-not-found adapter";
    # Defaults to pkgs.pi-command-not-found-adapter, which the flake's
    # overlays.default provides; the package is not in nixpkgs.
    package = mkPackageOption pkgs "pi-command-not-found-adapter" { };
  };

  config = mkIf cfg.enable {
    # home-manager keeps a package's share dirs off XDG_DATA_DIRS, so unlike
    # the NixOS module this one sources the hook for every shell explicitly,
    # each gated on the programs.* module the user enabled.
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