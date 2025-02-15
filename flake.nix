{
  description = "A reuseable github workflow for nix flakes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    blueprint.url = "github:numtide/blueprint";
    blueprint.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = inputs: inputs.blueprint {
    inherit inputs;
    prefix = "nix";
    systems = [ "aarch64-linux" "x86_64-linux" ];
  };
}
