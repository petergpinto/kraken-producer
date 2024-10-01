from rust:1.77-buster as builder

COPY . . 
RUN cargo install --path .

FROM debian:bookworm-slim
RUN apt update && apt-get install -y libssl-dev openssl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/kraken_rust_producer /usr/local/bin/kraken_rust_producer
CMD ["kraken_rust_producer"]