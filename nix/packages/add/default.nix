{ flake, pkgs }:

let
  workflowFile = ".github/workflows/flake.yml";
in pkgs.writeShellApplication {
  name = "add";
  text = ''
    if [ -f ${workflowFile} ]; then
      echo "Workflow file \"${workflowFile}\" already exists" >&2
      exit 1
    fi

    mkdir -pv "$(dirname ${workflowFile})"
    cp -v ${flake}/template.yml ${workflowFile}
    chmod 600 ${workflowFile}
  '';
}

