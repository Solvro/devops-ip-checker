FROM docker.io/library/rust:alpine AS builder
# add only what's necessary
COPY --parents src/ Cargo.lock Cargo.toml /source/
WORKDIR /source
# compile
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=alpine_cargo_dir \
    --mount=type=cache,target=/source/target,id=ip_checker_target \
    cargo build --release --locked && \
    cp /source/target/release/ip-checker /

# prod image
FROM scratch
COPY --from=builder /ip-checker /ip-checker
ENTRYPOINT ["/ip-checker"]
