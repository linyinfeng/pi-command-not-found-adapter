{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";

    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";

    nix-github-actions.url = "github:nix-community/nix-github-actions";
    nix-github-actions.inputs.nixpkgs.follows = "nixpkgs";

  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } (
      { self, ... }:
      {
        systems = [
          "x86_64-linux"
          "aarch64-linux"
        ];
        imports = [
          inputs.treefmt-nix.flakeModule
        ];
        perSystem =
          {
            config,
            self',
            pkgs,
            ...
          }:
          {
            packages = {
              pi-command-not-found-adapter = pkgs.callPackage ../../package.nix { };
              default = config.packages.pi-command-not-found-adapter;
            };

            checks = {
              inherit (self'.packages) pi-command-not-found-adapter;

              # The Nix modules hand the adapter a JSON config file, so the
              # option names and the keys the adapter reads have to stay in
              # step: this fails if a renamed option stops being understood.
              config-file =
                let
                  vars = import ../../modules/options.nix { inherit (pkgs) lib; };
                  prompt = pkgs.writeText "config-check-prompt.md" "a marker only this file has";
                  settings = vars.configFile pkgs {
                    pi = null;
                    model = "some/model";
                    thinking = null;
                    piArgs = [ "--verbose" ];
                    sessionRoot = null;
                    systemPromptFile = [ prompt ];
                    mcat = null;
                    width = null;
                    retries = 3;
                    toolLines = null;
                    timeout = null;
                    trace = null;
                  };
                in
                pkgs.runCommand "config-file" { } ''
                  ${self'.packages.pi-command-not-found-adapter}/bin/command-not-found-agent \
                    --config ${settings} system-prompt > $out
                  grep -q "a marker only this file has" $out
                  grep -q "some/model" ${settings}
                '';

              # Every hook has to parse in its shell; `--ide-check` parses
              # nushell without running the file.
              hooks =
                pkgs.runCommand "hooks"
                  {
                    nativeBuildInputs = with pkgs; [
                      bash
                      fish
                      nushell
                      zsh
                    ];
                  }
                  ''
                    export HOME=$TMPDIR
                    bash -n ${../../shell/bash.sh}
                    zsh -n ${../../shell/zsh.zsh}
                    fish -n ${../../shell/fish.fish}
                    nu --no-config-file --ide-check 0 ${../../shell/nushell.nu}
                    touch $out
                  '';
            };

            treefmt = {
              projectRoot = ../..;
              projectRootFile = "Cargo.toml";
              programs = {
                actionlint.enable = true;
                nixfmt.enable = true;
                prettier.enable = true;
                rustfmt.enable = true;
                shellcheck.enable = true;
                taplo.enable = true;
              };
              # `prompts/` is what the model reads, not documentation: a
              # reflow there changes the prompt, so it is left alone.
              settings.formatter.prettier.excludes = [ "prompts/*" ];
            };

            devShells.default = pkgs.mkShell {
              inputsFrom = [ self'.packages.pi-command-not-found-adapter ];
              packages = with pkgs; [
                clippy
                rust-analyzer
                rustfmt
                config.treefmt.build.wrapper
                # The hooks are checked in these four shells.
                bash
                fish
                nushell
                zsh
              ];
            };
          };
        flake.githubActions = inputs.nix-github-actions.lib.mkGithubMatrix {
          checks = self.checks;
        };
      }
    );
}
