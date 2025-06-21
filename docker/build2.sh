#!/usr/bin/env bash
set -euo pipefail
cd $(dirname "$0") || exit 1
export OFFLINE=""
if [[ "$@" = *"--offline"* ]]; then
    OFFLINE="--offline"
fi
BASE_NAME="lux"

if [[ ! -d target ]]; then
    mkdir target
fi
function build() {
    echo building $1!
    nix build $OFFLINE --out-link target/"$BASE_NAME-$1" .#$BASE_NAME-$1
}
build cli
build lua51
build lua52
build lua53
build lua54