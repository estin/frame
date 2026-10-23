{
  description = "A markdown task tracker with a terminal UI and CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        package = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
        frame = pkgs.rustPlatform.buildRustPackage {
          pname = package.name;
          version = package.version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "--bin" "fr" ];
        };
      in {
        packages.default = frame;
        apps.default = {
          type = "app";
          program = "${frame}/bin/fr";
        };
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt rustPackages.clippy ];
        };
      });
}
