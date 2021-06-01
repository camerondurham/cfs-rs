# container from scratch - in rust

[Liz Rice](https://t.co/4N8mNLhqs0?amp=1) gave several fantastic talks at Docker
Con and other events named [Containers from Scratch](https://youtu.be/8fi7uSYlOdc).
In these talks, she very impressively live-codes a minimal container runtime
in about 100 lines of Golang.

I love Golang and use it a lot but am trying to learn Rust currently. Given that
Rust offers stronger safety guarantees and is a safer wrapper around the syscalls
and C ffi's required, I thought it was a valid choice to try to reimplement cfs.

In this repository, I've attempted to write a very minimal container runtime, based
Liz Rice's original Golang implementation.

Please know that I'm new to Rust and this is **not** idiomatic code. If you have
any suggestions, please ping me or send a PR!

Credit to [vas-quod](https://github.com/flouthoc/vas-quod) for examples and snippets on
how to use the [nix](https://github.com/nix-rust/nix) package.


## usage

This will only work on a Unix system. I developed in WSL2.

```bash
# build the Docker container
make build

# run a shell
make run

# run args in the mini-container!
cargo run args...
```