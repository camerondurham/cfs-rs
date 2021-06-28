use cfs::ContainerBuilder;
use std::iter::FromIterator;
use std::{env, process};

const DIR_NAME: &str = "container-fs";
const CONTAINER_NAME: &str = "cfs-container";
const MAX_PIDS: u8 = 20;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || (args.get(1).unwrap().eq("run") && args.len() < 3) {
        println!("error: you must provide at least one argument or two arguments with `cfs run`");
        process::exit(1);
    }

    let command = args.get(1).unwrap();

    let chroot_path = check_var(
        "CHROOT_PATH",
        Some(cfs::util::pwd_join(DIR_NAME).unwrap().as_ref()),
    );

    let container_name = check_var("CONTAINER_NAME", Some(CONTAINER_NAME));
    let max_pids = check_var_u8("MAX_PIDS", MAX_PIDS);

    let container = match &command[..] {
        "run" => ContainerBuilder::new()
            .args(Vec::from_iter(args[2..].iter().cloned()))
            .chroot_path(chroot_path)
            .hostname(&container_name)
            .cgroup_name(&container_name)
            .max_pids(max_pids)
            .create(),
        _ => {
            println!("command not recognized\n{}", usage());
            process::exit(1)
        }
    };

    container.run();
}

fn check_var(env_var_name: &str, default: Option<&str>) -> String {
    if env::var(env_var_name).is_ok() {
        env::var(env_var_name).unwrap()
    } else {
        match default {
            Some(value) => String::from(value),
            None => panic!("value must be provided for env var: {}", env_var_name),
        }
    }
}

fn check_var_u8(env_var_name: &str, default: u8) -> u8 {
    if env::var(env_var_name).is_ok() {
        match env::var(env_var_name).unwrap().parse::<u8>() {
            Ok(ret) => ret,
            Err(_) => default,
        }
    } else {
        default
    }
}

fn usage() -> String {
    format!(
        "usage: cfs run cmd arg1 arg2 ...\
    \n  you can set the following environment variables:\
    \n  CHROOT_PATH=<path to your root filesystem to run the process inside>\
    \n  CONTAINER_NAME=<name for container hostname>"
    )
}
