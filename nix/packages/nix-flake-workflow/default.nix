{ flake, inputs, pkgs }:

let
  craneLib = inputs.crane.mkLib pkgs;
in flake.lib.buildStaticPackage pkgs {
    pname = "nix-flake-workflow";
    version = "0.1.0";
    src = craneLib.cleanCargoSource flake;
}

