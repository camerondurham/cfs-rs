# containers from scratch - in rust

Minimal re-implementation of
[lizrice/containers-from-scratch](https://github.com/lizrice/containers-from-scratch)
in Rust.

## why?

[Liz Rice](https://github.com/lizrice) gave several fantastic talks at
DockerCon and other events named
[Containers from Scratch](https://youtu.be/8fi7uSYlOdc).  In these talks, she impressively
live-codes a minimal container runtime in about 100 lines of Golang.

Many important container tools such as
[docker/engine](https://github.com/docker/engine),
[opencontainers/runc](https://github.com/opencontainers/runc), etc are written
in Golang. Golang is a great tool for building and running containers and I love
the language. However, safely handling syscalls in the language can
sometimes be tricky. Rust offers a safe, memory-efficient and
memory-safe wrapper around the syscall C-bindings and enforces strict error handling.
Due to these safety improvements, I believe Rust is a good choice to
reimplement Liz Rice's cfs example. Also, I'm trying to find any excuse to write
Rust! 

In this repository, I've attempted to write a very minimal container runtime, based
Liz Rice's original Golang implementation.

> Note: I'm pretty new to Rust and this is **not** idiomatic code. If you have
> any suggestions, please 
> [send a PR](https://github.com/camerondurham/cfs-rs/pulls) or
> [ping me on Discord](https://discord.com/users/632337069955612703)!

**Credit:**
* [vas-quod](https://github.com/flouthoc/vas-quod) for examples and snippets on
how to use the [nix](https://github.com/nix-rust/nix) package.
* Liz Rice's [Containers from Scratch](https://youtu.be/8fi7uSYlOdc) talk
  * note: to see her Golang implementation, see [lizrice/containers-from-scratch](https://github.com/lizrice/containers-from-scratch)


## usage

This will only work on a Unix system. I developed in WSL2.

```bash
# build the Docker container
make build

# run a shell
make run

# run args in the mini-container!
cfs args...
```

## examples

How do you know this is working?

example 0: new hostname

```bash
# run hostname in the Docker container
root@e8f49cd2ff70:/home# hostname
e8f49cd2ff70

# run hostname in the cfs container: we've changed hostnames
root@e8f49cd2ff70:/home# cfs run hostname
cfs-container
```

example 1: isolated process view

```bash
# run ps in the container
root@cb3e7658f63f:/usr/src/cfs# ps
  PID TTY          TIME CMD
    1 pts/0    00:00:00 sh
    7 pts/0    00:00:00 bash
    8 pts/0    00:00:00 ps

# run ps in cfs: the container thinks cfs is PID 1
root@cb3e7658f63f:/home# cfs run ps
  PID TTY          TIME CMD
    1 ?        00:00:00 cfs
    2 ?        00:00:00 ps
```

example 2: restricted view of mounts (still need to fill in output)

```bash
# run mount in the container
root@cb3e7658f63f:/home# cfs run mount
```
