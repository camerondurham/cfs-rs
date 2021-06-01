all: run

build:
	docker build -t cfs-rs:v1 .

run: build
	docker run --privileged -it cfs-rs:v1

run-now:
	docker run --privileged -it cfs-rs:v1