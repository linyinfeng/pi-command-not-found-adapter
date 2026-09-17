{
  description = "Ask pi for shell code when the shell cannot find a command";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      inherit (nixpkgs) lib;
      inherit (lib) fix listToAttrs nameValuePair;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      mkPkgs = system: import nixpkgs { inherit system; };
      mkPackages =
        system:
        fix (packages: {
          pi-command-not-found-adapter = (mkPkgs system).callPackage ./package.nix { };
          default = packages.pi-command-not-found-adapter;
        });
    in
    {
      packages = listToAttrs (map (system: nameValuePair system (mkPackages system)) systems);

      checks = self.packages;

      overlays.default = final: _prev: {
        pi-command-not-found-adapter = final.callPackage ./package.nix { };
      };

      # Modules take pkgs from their evaluation context (NixOS / home-manager).
      nixosModules.default = import ./modules/nixos.nix;
      homeManagerModules.default = import ./modules/home-manager.nix;
    };
}
