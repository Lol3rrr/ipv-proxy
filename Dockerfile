FROM rust:1.90.0-trixie AS builder

WORKDIR /app/
COPY . .

RUN cargo build --release

FROM debian:trixie

WORKDIR /app/
COPY --from=builder /app/target/release/ipv-proxy /app/ipv-proxy

ENTRYPOINT ["/app/ipv-proxy"]
