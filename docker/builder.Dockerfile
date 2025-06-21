FROM nixos/nix AS builder
RUN nix-channel --update
RUN echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf
RUN echo "accept-flake-config = true" >> /etc/nix/nix.conf
WORKDIR /tmp/build
COPY .. .
RUN nix --accept-flake-config build
