#!/usr/bin/env bash

set -euo pipefail

cd $(dirname "$0") || exit 1

BASE_NAME="lux"

# Allow includes
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

function build_image() {
    local tag="$1"
    local file="$2.Dockerfile"
    if [[ -z "$2" ]]; then
        file="$tag.Dockerfile"
    fi
    if [[ -z "$tag" ]]; then
        echo "Error: No tag provided for building the Docker image."
        exit 1
    fi
    if [[ ! -f "$file" ]]; then
        echo "Error: Dockerfile '$file' does not exist."
        exit 1
    fi
    echo "Building Docker image: $BASE_NAME:$tag"
    docker build -t "$BASE_NAME:$tag" -f $file ..
}
if [[ -z "$@" ]]; then
    build_image latest lua51
    build_image lua51
    build_image lua52
    build_image lua53
    build_image lua54
    build_image luajit
else
    build_image "$@"
fi