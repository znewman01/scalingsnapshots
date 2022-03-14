{ pkgs, gitignore, pre-commit-hooks-lib, naersk-lib, rust, ... }:
let
  noCheck = _: { doCheck = false; };
  python = pkgs.python39.override {
    packageOverrides = _: super: {
      # failing on m1 mac
      cheroot = super.cheroot.overridePythonAttrs noCheck;
      httplib2 = super.httplib2.overridePythonAttrs noCheck;
    };
  };
in rec {
  src = gitignore.lib.gitignoreSource ./.;

  devTools = {
    inherit (pkgs) nixfmt nix-linter git age;
    inherit (pkgs) libiconv;
    rust = rust.toolchain;
    pythonEnvAnalysis = ssanalyze.dependencyEnv;
    pythonEnvLogs = sslogs.dependencyEnv;
    inherit (python.pkgs) poetry black;
    inherit (pkgs.nodePackages) pyright;
  };

  pre-commit-check = pre-commit-hooks-lib.run {
    inherit src;
    hooks = {
      shellcheck.enable = true;
      nixfmt.enable = true;
      nix-linter.enable = true;
      black.enable = true;
      do-not-commit = {
        enable = true;
        name = "If 'DO NOT COMMIT' is in any file, this check fails.";
        entry = ''bash -c '! grep -E "DO NOT (COMMIT|SUBMIT)" "$@"' --'';
        language = "system";
        excludes = [ "^default.nix" ]; # otherwise this file matches!
      };
      my-clippy = {
        enable = true;
        # Workaround for https://github.com/cachix/pre-commit-hooks.nix/issues/94
        entry = ''
          bash -c ' \
             export CARGO_HOME="$(mktemp -d)/cargo"
             cp --no-preserve=mode -r ${
               builtins.head sssim.builtDependencies
             }/.cargo/ "''${CARGO_HOME}"
             PATH=${rust.toolchain}/bin:${pkgs.gcc}/bin:$PATH \
               cargo clippy --release --features strict --offline -- --no-deps
             rm -rf "''${CARGO_HOME}"
          '
        '';
        pass_filenames = false;
        types = [ "file" "rust" ];
      };
      my-rustfmt = {
        enable = true;
        entry = ''
          bash -c ' \
             PATH=${rust.toolchain}/bin:${pkgs.gcc}/bin:$PATH \
             cargo fmt -- --check --color always
          '
        '';
        files = "\\.rs$";
        pass_filenames = false;
      };
    };
    tools = { inherit (python.pkgs) black; };
  };

  sssim = let
    isRustFile = src:
      let
        # need to curry for memoization to work
        srcNotIgnored = gitignore.lib.gitignoreFilter src;
      in path: type:
      # not gitignored
      srcNotIgnored path type
      # if in the root, must be one of the given files:
      && ((builtins.dirOf path) != builtins.toString ./. # in the root
        || builtins.elem (builtins.baseNameOf path) [
          "Cargo.toml"
          "Cargo.lock"
          "rust-toolchain.toml"
          "src"
        ]);
    rustSrc = pkgs.lib.cleanSourceWith rec {
      src = ./.;
      filter = isRustFile src;
      name = "rust-source";
    };
    cargoPackage = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
  in naersk-lib.buildPackage {
    pname = cargoPackage.name;
    inherit (cargoPackage) version;
    src = rustSrc;

    doCheck = true;
    # doctests broken for some reason
    cargoTestCommands = old: [ "${builtins.head old} --lib" ];

    # hacks for making clippy work in CI
    postInstall = ''
      cp -r $CARGO_HOME $out/.cargo
    '';
  };

  ssanalyze = pkgs.poetry2nix.mkPoetryApplication {
    inherit python;
    projectDir = ./analysis;
    checkPhase = "pytest";
  };

  sslogs = pkgs.poetry2nix.mkPoetryApplication {
    inherit python;
    projectDir = ./logparser;
    checkPhase = "pytest";
  };

  # The full build: simulator program and Python analysis tools.
  scalingsnapshots = pkgs.buildEnv {
    name = "scalingsnapshots";
    paths = [ sssim ssanalyze sslogs ];
  };

  data = pkgs.copyPathToStore ./data;

  e2e-pipeline = pkgs.stdenv.mkDerivation {
    inherit src;

    name = "run-ssnap";
    installPhase = ''
      PATH=${scalingsnapshots}/bin/:${pkgs.jq}/bin/:$PATH
      mkdir -p $out

      # Check that our fakedata matches the schema (for both Rust and Python).
      diff <(jq --sort-keys < ${data}/fakedata.json) <(dummy_logs | jq --sort-keys)
      diff <(jq --sort-keys < ${data}/fakedata.json) <(sslogs_test_format | jq --sort-keys)

      # Try running the entire pipeline
      cat ${data}/fakedata.json \
          | sslogs identity \
          | sssim \
          | ssanalyze --non-sensitive-data --output $out/
    '';
  };
}
