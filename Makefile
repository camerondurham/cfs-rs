all: run

build:
	docker build -t cfs-rs:v1 .

run: build
	docker run --privileged -it cfs-rs:v1

test: build
	docker run --privileged -it cfs-rs:v1 cargo run run ls -l