{ sources ? import ./sources.nix }:
let
  # default nixpkgs
  pkgs = import sources.nixpkgs { };

  # gitignore.nix 
  gitignoreSource =
    (import sources."gitignore.nix" { inherit (pkgs) lib; }).gitignoreSource;

  pre-commit-hooks = (import sources."pre-commit-hooks.nix");

  src = gitignoreSource ./..;

  rust = import ./rust.nix { inherit sources; };

  naersk = pkgs.callPackage sources.naersk {
    rustc = rust;
    cargo = rust;
  };
in {
  inherit pkgs src;

  # provided by shell.nix
  devTools = {
    inherit (pkgs) niv nixfmt;
    inherit (pre-commit-hooks) pre-commit;
    inherit rust;
  };

  # to be built by github actions
  ci = {
    pre-commit-check = pre-commit-hooks.run {
      inherit src;
      hooks = {
        shellcheck.enable = true;
        nixfmt.enable = true;
        nix-linter.enable = true;
      };
      # generated files
      excludes = [ "^nix/sources.nix$" ];
    };
  };

  # TODO: get from Cargo.toml?
  crate = naersk.buildPackage {
    inherit src;

    pname = "scalingsnapshots";
    version = "0.1";
  };
}
