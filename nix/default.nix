{ sources ? import ./sources.nix }:
let
  # default nixpkgs
  pkgs = import sources.nixpkgs { };

  # gitignore.nix
  gitignoreSource =
    (import sources."gitignore.nix" { inherit (pkgs) lib; }).gitignoreSource;

  pre-commit-hooks = (import sources."pre-commit-hooks.nix");

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
in rec {
  inherit pkgs rust;

  src = gitignoreSource ./..;

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
        # Really should override pre-commit-hooks tools to use my Rust version rather than cloning.
        my-black = {
          name = "black";
          entry = "${pkgs.python3Packages.black}/bin/black";
          types = [ "file" "python" ];
        };
        my-rustfmt = {
          enable = true;
          entry =
            "bash -c 'PATH=${rust}/bin ${rust}/bin/cargo fmt -- --check --color always'";
          pass_filenames = false;
          types = [ "file" "rust" ];
        };
        my-clippy = {
          enable = true;
          entry =
            "bash -c 'PATH=${rust}/bin ${rust}/bin/cargo clippy --features strict -- --no-deps'";
          pass_filenames = false;
          types = [ "file" "rust" ];
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

  # The full build: simulator program and Python analysis tools.
  scalingsnapshots = pkgs.buildEnv {
    name = "scalingsnapshots";
    # TODO: should be nativeBuildInputs once it lands in nixpkgs
    # https://github.com/NixOS/nixpkgs/commit/4f6ec19dbc322d7ce8df9108b76e0db79682353e
    buildInputs = [ ci.pre-commit-check ];
    paths = [ crate analysis ];
  };

  run = pkgs.stdenv.mkDerivation {
    inherit src;

    name = "run-ssnap";
    installPhase = ''
      PATH=${scalingsnapshots}/bin/:$PATH
      mkdir -p $out

      scalingsnapshots | ssanalyze --input - --output $out/
    '';
  };
}
