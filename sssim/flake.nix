{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, fenix, naersk, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        rust = fenix.packages.${system}.fromToolchainFile {
          dir = ./.;
          sha256 = "/nC+LSETp1A78j+uU7TcCHnmLgjEtcIm809GTnNNdYE=";
        };
        naersk-lib = naersk.lib.${system}.override {
          rustc = rust;
          cargo = rust;
        };
      in rec {
        packages.default = let
          cargoPackage =
            (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
        in naersk-lib.buildPackage {
          pname = cargoPackage.name;
          inherit (cargoPackage) version;
          nativeBuildInputs = [ pkgs.gnum4 ];
          root = ./.;
          doCheck = true;
          # Workaround for https://github.com/cachix/pre-commit-hooks.nix/issues/94
          postInstall = ''
            cp -r $CARGO_HOME $out/.cargo
          '';
        };
        apps.default = flake-utils.lib.mkApp { drv = packages.default; };
        devShells.default = pkgs.mkShell {
          buildInputs = packages.default.nativeBuildInputs
            ++ [ pkgs.rust-analyzer ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux
            [ pkgs.linuxKernel.packages.linux_5_15.perf ];
          RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";
        };
      });
}
