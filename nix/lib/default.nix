{ inputs, ... }:


let
  getMuslTarget = system: if system == "x86_64-linux" then "x86_64-unknown-linux-musl" else "aarch64-unknown-linux-musl";
  getMuslCraneLib = pkgs: (inputs.crane.mkLib pkgs).overrideToolchain (inputs.rust-overlay.packages.${pkgs.system}.rust.override {
    targets = [ (getMuslTarget pkgs.system) ];
  });
in {
  buildStaticPackage = pkgs: attrs: (getMuslCraneLib pkgs).buildPackage ({
    strictDeps = true;
    CARGO_BUILD_TARGET = getMuslTarget pkgs.system;
    CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
  } // attrs);
}

