use cfs::{CmdType, ContainerBuilder};
use std::iter::FromIterator;
use std::{env, process};

const DIR_NAME: &str = "container-fs";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("error: you must provide at least one argument");
        process::exit(1);
    }

    let command = args.get(1).unwrap();
    let chroot_path = cfs::util::pwd_join(DIR_NAME).unwrap();
    let container = match &command[..] {
        "run" => {
            if args.len() < 3 {
                println!("error: you must provide at least one argument to run");
                process::exit(1);
            }
            ContainerBuilder::new()
            .args(Vec::from_iter(args[2..].iter().cloned()))
            .cmd_type(CmdType::RUN)
            .chroot_path(chroot_path)
            .create()},
        "shell" => ContainerBuilder::new()
            .args(vec![String::from("/bin/bash")])
            .cmd_type(CmdType::SHELL)
            .chroot_path(chroot_path)
            .create(),
        _ => {
            println!("command not recognized");
            process::exit(1)
        }
    };

    // let container = Container::new(Vec::from_iter(args[2..].iter().cloned()));
    container.run();
}
