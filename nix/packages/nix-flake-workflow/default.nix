{ flake, pkgs }:

pkgs.pkgsStatic.rustPlatform.buildRustPackage {
  pname = "nix-flake-workflow";
  version = "0.1.0";
  cargoLock.lockFile = ../../../Cargo.lock;
  src = flake;
}
