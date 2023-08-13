FROM rust:alpine as builder
RUN apk update
RUN apk add build-base openssl-dev
WORKDIR /usr/src/fizzle
COPY . .
RUN RUSTFLAGS='-C target-feature=-crt-static' cargo install --path fizzle

FROM alpine:latest
RUN apk update
RUN apk add ca-certificates openssl libgcc
COPY --from=builder /usr/local/cargo/bin/fizzle /usr/local/bin/fizzle
ENTRYPOINT ["/usr/local/bin/fizzle", "/etc/fizzle/fizzle.yaml"]
