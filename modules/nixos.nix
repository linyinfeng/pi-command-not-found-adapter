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
    # Puts the binary on PATH; the system profile's share dirs land on
    # XDG_DATA_DIRS, so fish and nushell auto-load the hooks from
    # share/fish/vendor_conf.d and share/nushell/vendor/autoload with no
    # per-shell config. Bash and zsh have no vendor auto-load, so they are
    # sourced explicitly below, gated on the shells the system config
    # actually enables.
    environment.systemPackages = [ cfg.package ];

    programs.bash.interactiveShellInit = mkIf config.programs.bash.enable ''
      source ${cfg.package.passthru.shell.bash}
    '';
    programs.zsh.interactiveShellInit = mkIf config.programs.zsh.enable ''
      source ${cfg.package.passthru.shell.zsh}
    '';
  };
}