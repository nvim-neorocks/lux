FROM nixos/nix AS builder
WORKDIR /tmp/build
COPY .. .
RUN nix --extra-experimental-features "nix-command flakes" --accept-flake-config build
