use nix::{mount, sched, sys, sys::stat::*, unistd};
use std::{
    io::prelude::*,
    os::unix::prelude::AsRawFd,
    path::{Path, PathBuf},
};

use std::{env, fs, path, process, u8};

const CHROOT_DIR_NAME: &str = "container-fs";
const CONTAINER_HOSTNAME: &str = "cfs-container";
const STACK_SIZE: usize = 1024 * 1024;

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Some(command) = args.get(1) {
        match &command[..] {
            "run" => run(args.as_ref()),
            _ => {
                println!("command not recognized");
                process::exit(1)
            }
        }
    } else {
        println!("you must provide a command");
        process::exit(1)
    }
}

fn child_process(args: &Vec<String>, hostname: &str, chroot_dir: &str) -> isize {
    match sched::unshare(sched::CloneFlags::CLONE_NEWNS) {
        Ok(_) => {
            if env::var("DEBUG").is_ok() {
                println!("unshare successful")
            }
        }
        Err(err) => panic!("failed to unshare: {:?}", err),
    };

    set_hostname(hostname);
    set_cgroups();
    set_chroot(chroot_dir);
    mount_fs();

    // run command in isolated environment
    process::Command::new(&args[2])
        .args(&args[3..])
        .stdin(process::Stdio::inherit())
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .spawn()
        .expect("failed to spawn child process")
        .wait()
        .unwrap();

    unmount_post_run();
    return 0;
}

fn run(args: &Vec<String>) {
    println!("Running [parent]: {:?} as {}", &args[2..], process::id());
    if args.len() < 3 {
        panic!("error: you must provide 3 arguments to the run command\n  example: [program] run ls -l")
    }

    let clone_flags = sched::CloneFlags::CLONE_NEWUTS
        | sched::CloneFlags::CLONE_NEWPID
        | sched::CloneFlags::CLONE_NEWNS;

    let current_dir = match std::env::current_dir() {
        Ok(p) => p,
        Err(err) => panic!("error making path: {:?}", err),
    };
    let current_dir = match current_dir.as_os_str().to_str() {
        Some(s) => s,
        None => panic!("error getting current dir"),
    };

    let chroot_path = Path::new(current_dir).join(CHROOT_DIR_NAME);
    let chroot_path = match chroot_path.to_str() {
        Some(p) => p,
        None => panic!("failed to convert path"),
    };

    let child_process_box = Box::new(|| child_process(args, CONTAINER_HOSTNAME, chroot_path));
    let mut stack: [u8; STACK_SIZE] = [0; STACK_SIZE];
    match sched::clone(
        child_process_box,
        &mut stack,
        clone_flags,
        Some(nix::sys::signal::SIGCHLD as i32),
    ) {
        Ok(_) => {
            if env::var("DEBUG").is_ok() {
                println!("clone succeeded")
            }
        }
        Err(err) => panic!("clone failed: {:?}", err),
    };
}

fn set_cgroups() {
    let cgroups = path::Path::new("/sys/fs/cgroup");
    let pids = cgroups.join("pids").join("cfs");

    // should be permissions 0700: owner can read, write, execute
    let mkdir_flags = Mode::S_IRWXU | Mode::S_IRGRP | Mode::S_IWGRP | Mode::S_IROTH | Mode::S_IWOTH;

    match unistd::mkdir(&pids, mkdir_flags){
        Ok(_) => {},
        Err(err) => println!("warning from mkdir: {:?}", err)
    };

    let cgroup_file_flags = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IXUSR;

    // TODO: set as a constant
    let max_pids = "20";
    write_file(pids.join("pids.max"), max_pids, cgroup_file_flags);

    let notify_on_release = "1";
    write_file(
        pids.join("notify_on_release"),
        notify_on_release,
        cgroup_file_flags,
    );

    let cgroup_procs = unistd::getpid().to_string();
    write_file(
        pids.join("cgroup.procs"),
        cgroup_procs.as_ref(),
        cgroup_file_flags,
    );
}

fn write_file(path: PathBuf, content: &str, flags: Mode) {
    let mut file = match fs::File::create(&path.to_str().unwrap()) {
        Ok(file) => file,
        Err(err) => panic!("error creating file at path [{:?}]: {:?}", path, err),
    };

    file.write_all(content.as_bytes())
        .expect("error writing to file");

    sys::stat::fchmodat(
        Some(file.as_raw_fd()),
        &path,
        flags,
        sys::stat::FchmodatFlags::FollowSymlink,
    )
    .expect("failed to set permissions");
}

fn set_hostname(hostname: &str) {
    unistd::sethostname(hostname).expect("set hostname failed");
}

fn set_chroot(path: &str) {
    util::log_result(util::Result {
        result: unistd::chroot(path),
        task_message: "set chroot",
    });

    util::log_result(util::Result {
        result: unistd::chdir("/"),
        task_message: "change directory to /",
    });
}

fn mount_fs() {
    util::log_result(util::Result {
        result: mount::mount(
            Some("proc"),
            "proc",
            Some("proc"),
            mount::MsFlags::empty(),
            Some(""),
        ),
        task_message: "mount proc",
    });
}

fn unmount_post_run() {
    util::log_result(util::Result {
        result: mount::umount("/proc"),
        task_message: "unmounting proc",
    });
}

mod util {
    use std::env;
    pub struct Result<'a> {
        pub result: core::result::Result<(), nix::Error>,
        pub task_message: &'a str,
    }

    pub fn log_result<'a>(mr: Result<'a>) {
        match mr.result {
            Ok(_) => {
                if env::var("DEBUG").is_ok() {
                    println!("success: {}", mr.task_message)
                }
            }
            Err(err) => panic!("failure: error with {}: \n{}", mr.task_message, err),
        }
    }
}
