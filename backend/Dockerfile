FROM rust:1.82-slim AS builder

WORKDIR /usr/src/sss-backend
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /usr/src/sss-backend/target/release/sss-backend /app/sss-backend

# Install ca-certificates for HTTPS
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

EXPOSE 3000

CMD ["/app/sss-backend"]