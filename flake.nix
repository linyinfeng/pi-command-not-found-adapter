{
  description = "Ask pi for shell code when the shell cannot find a command";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      version = (nixpkgs.lib.importTOML ./Cargo.toml).package.version;
    in
    {
      # Modules take pkgs from their evaluation context (NixOS / home-manager).
      nixosModules.default = import ./modules/nixos.nix;
      homeManagerModules.default = import ./modules/home-manager.nix;

      # The adapter is not in nixpkgs; the overlay is what makes
      # pkgs.pi-command-not-found-adapter exist, which is the modules'
      # package default. Users add it like any other overlay.
      overlays.default = final: prev: {
        pi-command-not-found-adapter = self.packages.${final.system}.default;
      };

      packages = eachSystem (pkgs: rec {
        pi-command-not-found-adapter = pkgs.rustPlatform.buildRustPackage (finalAttrs: {
          pname = "pi-command-not-found-adapter";
          inherit version;
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          # The integration tests drive a fake pi, which is a shell script.
          nativeCheckInputs = [ pkgs.bash ];
          postInstall = ''
            shells=$out/share/pi-command-not-found-adapter/shell
            install -Dm644 shell/bash.sh "$shells/bash.sh"
            install -Dm644 shell/zsh.zsh "$shells/zsh.zsh"
            install -Dm644 shell/fish.fish "$shells/fish.fish"
            install -Dm644 shell/nushell.nu "$shells/nushell.nu"

            prompts=$out/share/pi-command-not-found-adapter/prompts
            install -Dm644 prompts/base.md "$prompts/base.md"
            install -Dm644 prompts/nix.md "$prompts/nix.md"

            # fish and nushell autoload these from XDG_DATA_DIRS.
            install -Dm644 shell/fish.fish \
              $out/share/fish/vendor_conf.d/pi-command-not-found-adapter.fish
            install -Dm644 shell/nushell.nu \
              $out/share/nushell/vendor/autoload/pi-command-not-found-adapter.nu

            # bash and zsh login shells read /etc/profile.d.
            install -Dm644 shell/bash.sh \
              $out/etc/profile.d/pi-command-not-found-adapter.sh
          '';
          passthru.shell = {
            bash = "${finalAttrs.finalPackage}/share/pi-command-not-found-adapter/shell/bash.sh";
            fish = "${finalAttrs.finalPackage}/share/pi-command-not-found-adapter/shell/fish.fish";
            nushell = "${finalAttrs.finalPackage}/share/pi-command-not-found-adapter/shell/nushell.nu";
            zsh = "${finalAttrs.finalPackage}/share/pi-command-not-found-adapter/shell/zsh.zsh";
          };
          passthru.prompts = {
            base = "${finalAttrs.finalPackage}/share/pi-command-not-found-adapter/prompts/base.md";
            nix = "${finalAttrs.finalPackage}/share/pi-command-not-found-adapter/prompts/nix.md";
          };
          meta = {
            mainProgram = "command-not-found-agent";
            platforms = pkgs.lib.platforms.linux;
          };
        });
        default = pi-command-not-found-adapter;
      });

      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            clippy
            rust-analyzer
            rustc
            rustfmt
          ];
        };
      });

      checks = eachSystem (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) pi-command-not-found-adapter;
      });
    };
}
