{ project ? import ./nix { } }:

project.pkgs.mkShell {
  buildInputs = builtins.attrValues project.devTools;
  RUST_SRC_PATH = "${project.rust}/lib/rustlib/src/rust/library";
  shellHook = ''
    ${project.ci.pre-commit-check.shellHook}
  '';
}
