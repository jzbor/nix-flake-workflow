{
  description = "A reuseable github workflow for nix flakes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    cf.url = "github:jzbor/cornflakes";
    cf.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, cf }: cf.lib.flakeForDefaultSystems (system:
  let
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    apps = {
      add = let
        workflowFile = ".github/workflows/flake.yml";
        script = pkgs.writeShellApplication {
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
      in {
        type = "app";
        program = "${script}/bin/add";
      };

      # This mainly suits NixOS-based installations of attic
      create-attic-token = let
        script = pkgs.writeShellApplication {
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
      in {
        type = "app";
        program = "${script}/bin/create-attic-token";
      };
    };
  });
}
