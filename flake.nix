{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    sslogs.url = "path:logparser";
    ssanalyze.url = "path:analysis";
    generate-package-data.url = "path:generate-package-data";
    sssim.url = "path:sssim";
  };
  outputs = inputs@{ nixpkgs, fenix, pre-commit-hooks, flake-utils, sssim
    , sslogs, ssanalyze, generate-package-data, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        rust = fenix.packages.${system}.fromToolchainFile {
          file = ./sssim/rust-toolchain.toml;
          sha256 = "sha256-rSeLZ/Kx5HiZYq+tsDtWPPktbGKhodWCPryRG6CZSxU=";
        };
      in rec {
        apps = {
          sssim = sssim.apps.${system}.default;
          sslogs = sslogs.apps.${system}.default;
          ssanalyze = ssanalyze.apps.${system}.default;
          generate-package-data = generate-package-data.apps.${system}.default;
        };

        packages = rec {
          sssim = inputs.sssim.packages.${system}.default;
          sslogs = inputs.sslogs.packages.${system}.default;
          ssanalyze = inputs.ssanalyze.packages.${system}.default;
          generate-package-data =
            inputs.generate-package-data.packages.${system}.default;
          default = pkgs.buildEnv {
            name = "scalingsnapshots";
            paths = [ sssim ssanalyze sslogs ];
            pathsToLink = [ "/bin" ];
          };
        };

        checks.pre-commit-check = pre-commit-hooks.lib.${system}.run rec {
          src = ./.;
          hooks = {
            nixfmt.enable = true;
            statix.enable = true;
            black.enable = true;
            do-not-commit = {
              enable = true;
              name = "If 'DO NOT COMMIT' is in any file, this check fails.";
              entry = ''bash -c '! grep -E "DO NOT (COMMIT|SUBMIT)" "$@"' --'';
              language = "system";
              excludes = [ "^flake.nix" ]; # otherwise this file matches!
            };
            clippy = {
              enable = true;
              # Workaround for https://github.com/cachix/pre-commit-hooks.nix/issues/94
              entry = let
                sssimDeps = builtins.head
                  sssim.packages.${system}.default.builtDependencies;
              in pkgs.lib.mkForce ''
                bash -c ' \
                  export CARGO_HOME="$(mktemp -d)/cargo"
                  cp --no-preserve=mode -r ${sssimDeps}/.cargo/ "''${CARGO_HOME}"
                  ${tools.cargo}/bin/cargo clippy --release --features strict --offline -- --no-deps
                  rm -rf "''${CARGO_HOME}"
                '
              '';
            };
            rustfmt = {
              enable = true;
              entry = pkgs.lib.mkForce ''
                ${tools.rustfmt}/bin/cargo-fmt fmt --manifest-path sssim/Cargo.toml -- --check --color always
              '';
            };
          };
          tools = let
            wrappedRust = pkgs.symlinkJoin {
              name = "wrapped-rust";
              paths = [ rust ];
              buildInputs = [ pkgs.makeWrapper ];
              postBuild = ''
                for p in $out/bin/*; do
                  wrapProgram $p --prefix PATH : ${rust}/bin
                done
              '';
            };
          in {
            inherit (pkgs.python39) black;
            cargo = wrappedRust;
            rustfmt = wrappedRust;
          };
        };

        checks.e2e-pipeline = let data = pkgs.copyPathToStore ./data;
        in pkgs.stdenv.mkDerivation {
          name = "e2e-pipeline";
          dontUnpack = true;
          buildCommand = ''
            PATH=${packages.default}/bin/:${pkgs.jq}/bin/:$PATH
            mkdir -p $out

            echo "Check that our fakedata matches the schema (for Rust)."
            diff <(jq --sort-keys < ${data}/fakedata.json) <(dummy_logs | jq --sort-keys)
            echo "Check that our fakedata matches the schema (for Python)."
            diff <(jq --sort-keys < ${data}/fakedata.json) <(sslogs_test_format | jq --sort-keys)

            # Try running the entire pipeline
            cat ${data}/fakedata.json \
                | sslogs identity \
                > $out/events.json

            sssim --events-path $out/events.json --init-path ${data}/fakedata-initial.json \
                | ssanalyze --non-sensitive-data --output $out/
          '';
        };
        devShells.default = pkgs.mkShell {
          buildInputs = [ pkgs.age pkgs.statix pkgs.nixfmt ]
            ++ checks.pre-commit-check.nativeBuildInputs
            ++ sslogs.devShells.${system}.default.buildInputs
            ++ ssanalyze.devShells.${system}.default.buildInputs
            ++ sssim.devShells.${system}.default.buildInputs;
          RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";
          shellHook = checks.pre-commit-check.shellHook + ''
            export CARGO_HOME=$PWD/.cargo
          '';
        };
      });
}
