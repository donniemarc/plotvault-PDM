# syntax=docker/dockerfile:1

# ---------- build stage ----------
FROM rust:1.97-bookworm AS builder
WORKDIR /app

# build deps for libredwg (provides dwg2dxf for DWG -> DXF conversion)
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        cmake \
        zlib1g-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# libredwg is not packaged in Debian main, so build from source.
# Note: libredwg 0.14 ships a generated `configure` (autotools), NOT a CMakeLists.txt.
ADD https://github.com/LibreDWG/libredwg/releases/download/0.14/libredwg-0.14.tar.gz /tmp/libredwg.tar.gz
RUN tar -xzf /tmp/libredwg.tar.gz -C /tmp \
    && cd /tmp/libredwg-0.14 \
    && ./configure --prefix=/tmp/libredwg-install \
        --disable-shared --enable-static \
        --disable-bindings --disable-docs --disable-werror \
    && make -j"$(nproc)" \
    && make install

# build plotvault-pdm server
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

# ---------- runtime stage ----------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /tmp/libredwg-install/bin/dwg2dxf /usr/local/bin/dwg2dxf
COPY --from=builder /app/target/release/plotvault-pdm-server /usr/local/bin/plotvault-pdm-server

ENV DATA_DIR=/data
ENV CONFIG_DIR=/config
ENV BIND=0.0.0.0:8642
EXPOSE 8642
VOLUME ["/data", "/config"]

CMD ["plotvault-pdm-server"]
