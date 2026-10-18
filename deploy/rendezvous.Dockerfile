# The rendezvous server as a container: what `relay remote via wss://…` points at.
#
#   docker build -f deploy/rendezvous.Dockerfile -t relay-rendezvous .
#   docker run -d --name relay-rendezvous -p 127.0.0.1:7430:7430 \
#     -e RELAY_RENDEZVOUS_ROOMS=<your room id>,<another> relay-rendezvous
#
# Put TLS in front (docs/MOBILE.md §4); the container itself speaks plain ws:// on 7430.
# Only the headless crates are built, so no GTK is needed.
#
# Published on loopback as above, the container is reached only through the proxy on the host,
# and every connection comes from the Docker bridge's gateway, which the per-address caps never
# count. That proxy (Caddy's reverse_proxy, or nginx with proxy_add_x_forwarded_for) writes the
# phone's address into X-Forwarded-For, so the image trusts one proxy and counts that address.
# Behind two proxies set -e RELAY_RENDEZVOUS_TRUST_PROXY=2 (and see MOBILE.md §4 for when);
# exposed with no proxy in front, -e RELAY_RENDEZVOUS_TRUST_PROXY=0, or any phone could write
# the header and pick the address it is counted as. RELAY_RENDEZVOUS_ROOMS lists the rooms
# served: without it, strangers can make up rooms of their own (the server warns at start).
#
# This image is also where the bot's container copies its relay binary from (relay-server-image.yml);
# a COPY takes the file only, never the ENV below.

FROM rust:1.99-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/relay-native/Cargo.toml ./apps/relay-native/Cargo.toml
RUN mkdir -p apps/relay-native/src && echo 'fn main() {}' > apps/relay-native/src/main.rs
RUN cargo build --release --locked -p relay-cli

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/relay /usr/local/bin/relay
USER 65534:65534
EXPOSE 7430
ENV RELAY_LOG=info
ENV RELAY_RENDEZVOUS_TRUST_PROXY=1
ENTRYPOINT ["relay", "remote", "rendezvous", "--bind", "0.0.0.0:7430"]
