# Build the application
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

ADD . /build
WORKDIR /build

RUN cargo build --release


# Host the application
FROM scratch

COPY --from=builder /build/target/release/app-controller /app-controller

CMD ["/app-controller"]
