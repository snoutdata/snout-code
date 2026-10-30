# The toolchain SnoutCode for Linux is built, tested and packaged with (scripts/package-linux.sh,
# scripts/linux/test.sh): Ubuntu 22.04, so the binary needs no newer glibc (2.35) than that,
# with Rust, the libraries GPUI links for X11 and Wayland, Zig 0.15 for the Ghostty terminal
# engine, Temurin JDK 21 for the Java engine (its runtime is jlinked from it), dpkg-deb, and
# Xvfb, sway and Mesa's software renderers to start the app with no display.
FROM ubuntu:22.04
ARG RUST_VERSION=1.99.1
ARG ZIG_VERSION=0.15.2
ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/opt/jdk21/bin:$PATH \
    JAVA_HOME=/opt/jdk21
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl git build-essential pkg-config cmake python3 python3-pip \
        libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev \
        libfontconfig-dev libfreetype-dev libvulkan1 mesa-vulkan-drivers libgl1-mesa-dri libegl1 \
        xvfb xauth sway grim xdotool x11-utils imagemagick dbus dbus-x11 \
        dpkg-dev fakeroot rsync zstd file unzip jq librsvg2-bin desktop-file-utils \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION" \
        --component clippy,rustfmt \
    && chmod -R a+rwX "$CARGO_HOME" "$RUSTUP_HOME"
RUN pip3 install --no-cache-dir "ziglang==$ZIG_VERSION" \
    && ln -s "$(python3 -c 'import ziglang, os; print(os.path.dirname(ziglang.__file__))')/zig" /usr/local/bin/zig \
    && zig version
# Temurin 21 for this architecture, checked against the SHA-256 Adoptium publishes.
RUN set -eu; \
    case "$(uname -m)" in aarch64) a=aarch64 ;; *) a=x64 ;; esac; \
    curl -fsSL -o /tmp/assets.json "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=$a&image_type=jdk&os=linux&vendor=eclipse"; \
    link="$(jq -r '.[0].binary.package.link' /tmp/assets.json)"; sum="$(jq -r '.[0].binary.package.checksum' /tmp/assets.json)"; \
    curl -fsSL -o /tmp/jdk.tar.gz "$link"; echo "$sum  /tmp/jdk.tar.gz" | sha256sum -c -; \
    mkdir -p /opt/jdk21 && tar -xzf /tmp/jdk.tar.gz -C /opt/jdk21 --strip-components=1; \
    rm /tmp/jdk.tar.gz /tmp/assets.json; java -version
