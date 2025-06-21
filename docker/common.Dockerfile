INCLUDE+ docker/postBuild.Dockerfile

FROM scratch AS common
WORKDIR /app
# Ensure that lx is available in the PATH
COPY --from=builder /tmp/nix-store-closure /nix/store
COPY --from=builder /tmp/build/result /build