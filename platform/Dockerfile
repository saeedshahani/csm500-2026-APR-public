# Build remotely with Cloud Build or on a CI runner; local Docker is optional.
FROM rust:1.85.0-bookworm@sha256:0ff31c9ffa641a62e48d543fb00b4960955ea375f40776f40f585b89e654cc5e AS native
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY examples ./examples
RUN cargo build --locked --release -p cli

FROM node:24.21.0-bookworm-slim@sha256:0e0ff40c39bc087845bfb27465a0df4ea419520094bc35842ff83dd8cbe6f9b6
RUN apt-get update && apt-get install --no-install-recommends -y util-linux ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY platform/package.json platform/package-lock.json ./platform/
RUN npm ci --prefix platform --omit=dev --ignore-scripts && npm cache clean --force
COPY --from=native /build/target/release/cli ./target/release/cli
COPY platform ./platform
COPY examples ./examples
RUN mkdir -p /app/output && chown node:node /app/output
USER node
ENV GRAPH_NATIVE_MEMORY_BYTES=536870912
ENV GRAPH_NATIVE_CPU_SECONDS=30
# The default is intentionally the loopback/local adapter, not a cloud deployment.
# A hosted adapter must supply durable storage and authenticated dispatch first.
CMD ["node", "platform/server.mjs"]
