{ sources ? import ./sources.nix }:
let
  pkgs =
    import sources.nixpkgs { overlays = [ (import sources.nixpkgs-mozilla) ]; };
  channel = "nightly";
  date = "2021-09-22";
  extensions = [ "rust-analyzer-preview" "clippy-preview" "rust-src" ];
  chan = (pkgs.rustChannelOf { inherit channel date; }).rust.override {
    inherit extensions;
  };
in chan
