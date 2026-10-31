# aster-gateway — multi-stage image built from this package root
#
#   docker build -t aster-gateway:local .
#   # the OS compose file builds this as service `aster-gateway`
#
# Base images use AWS Public ECR mirrors of Docker Hub library images
# (avoids intermittent registry-1.docker.io failures).

FROM public.ecr.aws/docker/library/rust:1.88-bookworm AS builder
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates git \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY tests ./tests
COPY benches ./benches
COPY examples ./examples
COPY scripts ./scripts
# WORKDIR is /src, so Cargo's ../../crates/<name> resolves at /crates/<name>.
COPY --from=apofasi . /crates/apofasi
COPY --from=bge . /crates/bge
COPY --from=ocr . /crates/ocr
RUN cargo build --locked --release --bin aster-gateway

FROM public.ecr.aws/docker/library/debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 gateway
COPY --from=builder /src/target/release/aster-gateway /usr/local/bin/aster-gateway
USER gateway
WORKDIR /home/gateway
EXPOSE 8080 9090
HEALTHCHECK --interval=10s --timeout=5s --start-period=15s --retries=5 \
  CMD wget -qO- http://127.0.0.1:9090/health || exit 1
ENTRYPOINT ["aster-gateway"]
