all: build run

build:
	docker build -t cfs-rs:v1 .

run:
	docker run --privileged -it cfs-rs:v1