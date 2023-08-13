FROM ubuntu:latest

RUN apt update && apt install --yes openssl ca-certificates

WORKDIR /
RUN mkdir /app
RUN mkdir /etc/fizzle

COPY --chmod=755 target/release/fizzle /app/fizzle

ENTRYPOINT ["/app/fizzle", "/etc/fizzle/fizzle.yaml"]
