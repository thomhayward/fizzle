FROM rust:alpine as builder
RUN apk update
RUN apk add build-base libressl-dev
WORKDIR /usr/src/fizzle
COPY . .
RUN cargo install --path fizzle

FROM alpine:latest
COPY --from=builder /usr/local/cargo/bin/fizzle /usr/local/bin/fizzle
ENTRYPOINT ["/usr/local/bin/fizzle", "/etc/fizzle/fizzle.yaml"]
