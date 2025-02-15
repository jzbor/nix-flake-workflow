{ pkgs }:

# This mainly suits NixOS-based installations of attic
pkgs.writeShellApplication {
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
}

