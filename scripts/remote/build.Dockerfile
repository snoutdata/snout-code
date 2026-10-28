# The toolchain scripts/build-server-linux.sh builds snout-server with: Rust, zig as the
# linker, and cargo-zigbuild, which cross compiles static musl binaries for x86_64 and
# aarch64 Linux from either kind of host.
FROM rust:1.99-slim-bookworm
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3-pip \
    && pip3 install --no-cache-dir --break-system-packages ziglang==0.15.2 \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked cargo-zigbuild@0.24.0 \
    && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
