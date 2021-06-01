FROM rust:1.52-slim

WORKDIR /usr/src/cfs
COPY . .

RUN cargo install --path . && cargo build

CMD /bin/bash