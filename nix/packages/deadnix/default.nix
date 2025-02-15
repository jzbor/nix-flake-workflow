{ flake, pkgs }:

flake.lib.buildStaticPackage pkgs {
    pname = "deadnix";
    inherit (pkgs.deadnix) version src;
    # cargoLock = "${pkgs.deadnix.src}/Cargo.lock";
}

