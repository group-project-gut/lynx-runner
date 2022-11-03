FROM docker.io/rust:1-slim-buster AS runner-builder

COPY ./ /runner/
WORKDIR /runner

#RUN apt-get install pkg-config musl-tools
#RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release

FROM quay.io/podman/stable:latest

COPY --from=runner-builder /runner/ /runner/
WORKDIR /runner

ENTRYPOINT [ "./entrypoint.sh" ]