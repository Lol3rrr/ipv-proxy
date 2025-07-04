FROM rust:1.88.0 AS builder

WORKDIR /app/
COPY . .

RUN cargo build --release

ENTRYPOINT ["/app/target/release/ipv-proxy"]
