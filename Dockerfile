FROM ubuntu:20.04 as rootfs
ENV TESTING_PROGRAMS="gcc g++"
RUN apt-get update && apt-get install -y ${TESTING_PROGRAMS}

FROM rust:1.53 as builder
COPY --from=rootfs / /home/container-fs
WORKDIR /usr/src/cfs
COPY . .
RUN cargo install --path . \
  && cargo build \
  && touch /DOCKER_ROOT_DIR \
  && touch /home/container-fs/CONTAINER_ROOT_DIR \
  && cp /usr/src/cfs/target/release/cfs /usr/local/bin \
  && mv /usr/src/cfs/check /home/container-fs
WORKDIR /home
CMD /bin/bash