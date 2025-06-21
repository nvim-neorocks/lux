# syntax=aminya/dockerfile-plus
# root paths because docker
INCLUDE+ docker/builder.Dockerfile

RUN nix --accept-flake-config build .#lux-lua51

INCLUDE+ docker/common.Dockerfile

ENTRYPOINT [ "/bin/sh" ]