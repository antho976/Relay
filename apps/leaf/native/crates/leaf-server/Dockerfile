# leaf-server's image. Built from apps/leaf/native (the build context), so the workspace's
# Cargo.lock pins every dependency: docker compose -f apps/leaf/server/compose.yml build
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p leaf-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/leaf-server /usr/local/bin/leaf-server
ENV LEAF_DATA=/data LEAF_BIND=0.0.0.0:4580
VOLUME /data
EXPOSE 4580
USER 1000:1000
CMD ["leaf-server"]
