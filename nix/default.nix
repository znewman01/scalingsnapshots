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

  # if we try to gitignore this source we get infinite recursion; it gets
  # cleaned in poetry2nix
  analysisPath = ./../analysis;
  poetryArgs = {
    projectDir = analysisPath;
    python = pkgs.python38;
  };
in {
  inherit pkgs src rust;

  # provided by shell.nix
  devTools = {
    inherit (pkgs) niv nixfmt nix-linter git;
    inherit (pre-commit-hooks) pre-commit;
    inherit rust;
    pythonEnv = pkgs.poetry2nix.mkPoetryEnv poetryArgs;
    inherit (pkgs.python38Packages) poetry;
    inherit (pkgs.nodePackages) pyright;
  };

  # to be built by github actions
  ci = {
    pre-commit-check = pre-commit-hooks.run {
      inherit src;
      hooks = {
        shellcheck.enable = true;
        nixfmt.enable = true;
        nix-linter.enable = true;
        black.enable = true;
        # Really should override pre-commit-hooks tools to use my Rust version rather than cloning.
        my-rustfmt = {
          enable = true;
          entry =
            "bash -c 'PATH=${rust}/bin ${rust}/bin/cargo fmt -- --check --color always'";
          pass_filenames = false;
          files = "\\.rs$";
        };
        my-clippy = {
          enable = true;
          entry =
            "bash -c 'PATH=${rust}/bin ${rust}/bin/cargo clippy --features strict -- --no-deps'";
          pass_filenames = false;
          files = "\\.rs$";
        };
        do-not-commit = {
          enable = true;
          name = "If 'DO NOT COMMIT' is in any file, this check fails.";
          entry = ''bash -c '! grep "DO NOT COMMIT" "$@"' --'';
          language = "system";
          excludes = [ "^nix/default.nix" ]; # otherwise this file matches!
        };
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

    doCheck = true; # run `cargo test`
  };

  analysis = pkgs.poetry2nix.mkPoetryApplication
    (poetryArgs // { checkPhase = "pytest"; });
}
