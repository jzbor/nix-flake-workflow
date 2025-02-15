{ flake, pkgs }:

flake.lib.buildStaticPackage pkgs {
    pname = "statix";
    inherit (pkgs.statix) version src;
    # cargoLock = "${pkgs.statix.src}/Cargo.lock";
}

