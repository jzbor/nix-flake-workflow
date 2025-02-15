{ flake, inputs, pkgs }:

let
  craneLib = inputs.crane.mkLib pkgs;
in craneLib.buildPackage {
  pname = "nix-flake-workflow";
  version = "0.1.0";
  src = craneLib.cleanCargoSource flake;
}

