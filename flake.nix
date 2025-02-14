{
  description = "A reuseable github workflow for nix flakes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    cf.url = "github:jzbor/cornflakes";
    cf.inputs.nixpkgs.follows = "nixpkgs";

    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, cf, crane }: cf.lib.flakeForDefaultSystems (system:
  let
    pkgs = nixpkgs.legacyPackages.${system};
    craneLib = crane.mkLib pkgs;
  in {
    packages = rec {
      default = nix-flake-workflow;
      nix-flake-workflow = craneLib.buildPackage {
        pname = "nix-flake-workflow";
        version = "0.1.0";
        src = craneLib.cleanCargoSource ./.;
      };

      add = let
        workflowFile = ".github/workflows/flake.yml";
      in pkgs.writeShellApplication {
        name = "add";
        text = ''
          if [ -f ${workflowFile} ]; then
            echo "Workflow file \"${workflowFile}\" already exists" >&2
            exit 1
          fi

          mkdir -pv "$(dirname ${workflowFile})"
          cp -v ${self}/template.yml ${workflowFile}
          chmod 600 ${workflowFile}
        '';
      };


      # This mainly suits NixOS-based installations of attic
      create-attic-token = pkgs.writeShellApplication {
        name = "create-attic-token";
        text = ''
          if [ "$#" -lt 1 ]; then
            echo "Usage: $0 <ssh-host> <cache-name>"
            exit 1
          fi

          # shellcheck disable=SC2029
          token="$(ssh "$1" "cd / && atticd-atticadm make-token --sub github-ci --validity 2y --pull $2 --push $2 2> /dev/null")"

          echo "ATTIC_ENDPOINT: <your-endpoint>"
          echo "ATTIC_CACHE: $2"
          echo "ATTIC_TOKEN: $token"
        '';
      };
    };
  });
}
