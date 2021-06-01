FROM ubuntu:20.04 as rootfs
FROM rust:1.52 as builder

COPY --from=rootfs / /home/container-fs

WORKDIR /usr/src/cfs
COPY . .

RUN cargo install --path . \
  && cargo build \
  && touch /DOCKER_ROOT_DIR \
  && touch /home/container-fs/CONTAINER_ROOT_DIR \
  && cp /usr/src/cfs/target/release/cfs /usr/local/bin

WORKDIR /home
CMD /bin/bash