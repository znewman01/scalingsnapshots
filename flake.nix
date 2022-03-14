{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    gitignore = {
      url = "github:hercules-ci/gitignore.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    poetry2nix = {
      url = "github:nix-community/poetry2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, gitignore, fenix, pre-commit-hooks, naersk, flake-utils
    , poetry2nix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ poetry2nix.overlay ];
        };
        rust = fenix.packages.${system}.latest;
        naersk-lib =
          naersk.lib.${system}.override { inherit (rust) rustc cargo; };
        pre-commit-hooks-lib = pre-commit-hooks.lib.${system};
        project = import ./. {
          inherit pkgs gitignore naersk-lib rust pre-commit-hooks-lib;
        };
      in {
        apps = {
          sssim = flake-utils.lib.mkApp { drv = project.sssim; };
          sslogs = flake-utils.lib.mkApp { drv = project.sslogs; };
          ssanalyze = flake-utils.lib.mkApp { drv = project.ssanalyze; };
        };
        devShells.default = pkgs.mkShell {
          buildInputs = builtins.attrValues project.devTools;
          RUST_SRC_PATH = "${rust.rust-src}/lib/rustlib/src/rust/library";
          shellHook = project.pre-commit-check.shellHook + ''
            export CARGO_HOME=$PWD/.cargo
          '';
        };
        checks = { inherit (project) pre-commit-check; };
        packages = { inherit (project) e2e-pipeline; };
      });
}
