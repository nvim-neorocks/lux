# syntax=aminya/dockerfile-plus
# absolute paths because docker
INCLUDE+ docker/builder.Dockerfile

RUN nix --extra-experimental-features "nix-command flakes" --accept-flake-config build .#lux-lua51

INCLUDE+ docker/common.Dockerfile

CMD [ "lx", "run" ]