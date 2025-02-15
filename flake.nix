{
  description = "A reuseable github workflow for nix flakes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    blueprint.url = "github:numtide/blueprint";
    blueprint.inputs.nixpkgs.url = "nixpkgs";

    crane.url = "github:ipetkov/crane";
  };

  outputs = inputs: inputs.blueprint {
    inherit inputs;
    prefix = "nix";
  };
}
