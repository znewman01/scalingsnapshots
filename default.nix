{ project ? import ./nix { } }:

project.pkgs.buildEnv {
  name = "scalingsnapshots";
  # TODO: should be nativeBuildInputs once it lands in nixpkgs
  # https://github.com/NixOS/nixpkgs/commit/4f6ec19dbc322d7ce8df9108b76e0db79682353e
  buildInputs = [ project.ci.pre-commit-check ];
  paths = [ project.crate project.analysis ];
}
