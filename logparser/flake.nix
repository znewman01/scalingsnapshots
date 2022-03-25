{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    poetry2nix = {
      url = "github:nix-community/poetry2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, flake-utils, poetry2nix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ poetry2nix.overlay ];
        };
        noCheck = _: { doCheck = false; };
        python = pkgs.python39.override {
          packageOverrides = _: super:
            pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
              # failing on m1 mac
              cheroot = super.cheroot.overridePythonAttrs noCheck;
              graphviz = super.graphviz.overridePythonAttrs noCheck;
            };
        };
      in rec {
        packages.default = pkgs.poetry2nix.mkPoetryApplication {
          inherit python;
          projectDir = ./.;
          checkPhase = "pytest";
        };
        apps = { default = flake-utils.lib.mkApp { drv = packages.default; }; };
        devShells.default = pkgs.mkShell {
          buildInputs = [
            packages.default.dependencyEnv
            python.pkgs.poetry
            python.pkgs.black
            python.pkgs.pytest
            pkgs.nodePackages.pyright
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
        };
      });
}
