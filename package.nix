{
  lib,
  rustPlatform,
  bash,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "pi-command-not-found-adapter";
  version = (lib.importTOML ./Cargo.toml).package.version;

  # Only what the build and its tests read, so the two flakes and a plain
  # `import ./.` all get the same source.
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.lock
      ./Cargo.toml
      ./prompts
      ./shell
      ./src
      ./tests
    ];
  };

  cargoLock.lockFile = ./Cargo.lock;

  # The integration tests drive a fake pi, which is a shell script.
  nativeCheckInputs = [ bash ];

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
    description = "Ask pi for a command when the shell cannot find one, and print the code to source";
    homepage = "https://github.com/linyinfeng/pi-command-not-found-adapter";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ yinfeng ];
    mainProgram = "command-not-found-agent";
    platforms = lib.platforms.unix;
  };
})
