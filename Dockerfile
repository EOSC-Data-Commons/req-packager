# ---- Build stage ----
FROM rust:1.90 AS builder

WORKDIR /app
COPY . .

RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

RUN cargo build --release --bin rp

# ---- Runtime stage ----
FROM debian:trixie-slim

WORKDIR /app

RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

# Copy binary (replace with your binary name)
COPY --from=builder /app/target/release/rp /usr/local/bin/rp

CMD ["rp"]
