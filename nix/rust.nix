{ sources ? import ./sources.nix }:
let
  pkgs =
    import sources.nixpkgs { overlays = [ (import sources.nixpkgs-mozilla) ]; };
  extensions = [ "rust-analyzer-preview" "clippy-preview" "rust-src" ];
  chan = (pkgs.rustChannelOf {
    # Mozilla overlay has no support for rust-toolchain.toml
    # https://github.com/mozilla/nixpkgs-mozilla/issues/245
    rustToolchain = ./../rust-toolchain;
  }).rust.override { inherit extensions; };
in chan
