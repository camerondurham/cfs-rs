use nix::{mount, sched, sys, sys::stat::*, unistd};
use std::{
    io::{prelude::*, BufReader},
    iter::FromIterator,
    os::unix::prelude::AsRawFd,
    path::PathBuf,
};

use std::process::{self, Command, Stdio};
use std::{env, fs, path, u8};

const CHROOT_DIR_NAME: &str = "container-fs";
const CONTAINER_HOSTNAME: &str = "cfs-container";
const MAX_PIDS: &str = "20";
const NOTIFY_ON_RELEASE: &str = "1";

pub enum CmdType {
    RUN,
    SHELL,
}

pub struct Container {
    pub args: Vec<String>,
    pub cmd_type: CmdType,
    pub debug: bool,
    chroot_path: String,
}

impl Container {
    pub fn new(args: &Vec<String>) -> Self {
        if let Some(command) = args.get(1) {
            match &command[..] {
                "run" => {
                    return Container {
                        args: Vec::from_iter(args[2..].iter().cloned()),
                        cmd_type: CmdType::RUN,
                        debug: util::in_debug_mode(),
                        chroot_path: filesystem::join_current_dir(CHROOT_DIR_NAME).expect(
                            format!("failed to find chroot dir: {:?}", CHROOT_DIR_NAME).as_str(),
                        ),
                    };
                }
                "shell" => {
                    return Container {
                        args: vec![String::from("/bin/bash")],
                        cmd_type: CmdType::SHELL,
                        debug: util::in_debug_mode(),
                        chroot_path: filesystem::join_current_dir(CHROOT_DIR_NAME).expect(
                            format!("failed to find chroot dir: {:?}", CHROOT_DIR_NAME).as_str(),
                        ),
                    };
                }
                _ => {
                    println!("command not recognized");
                    process::exit(1)
                }
            }
        } else {
            panic!("error: you must provide at least two arguments");
        }
    }

    pub fn run(self: Self) {
        println!(
            "Running [parent]: {:?} as {}",
            &self.args[..],
            process::id()
        );

        let clone_flags = sched::CloneFlags::CLONE_NEWUTS
            | sched::CloneFlags::CLONE_NEWPID
            | sched::CloneFlags::CLONE_NEWNS;

        let child_process_box =
            Box::new(|| child_process(&self.args, CONTAINER_HOSTNAME, self.chroot_path.as_str()));
        const STACK_SIZE: usize = 1024 * 1024;
        let mut stack: [u8; STACK_SIZE] = [0; STACK_SIZE];
        match sched::clone(
            child_process_box,
            &mut stack,
            clone_flags,
            Some(nix::sys::signal::SIGCHLD as i32),
        ) {
            Ok(_) => {
                if self.debug {
                    println!("clone succeeded")
                }
            }
            Err(err) => panic!("clone failed: {:?}", err),
        };
    }
}

// run user input command in child process
fn child_process(args: &Vec<String>, hostname: &str, chroot_dir: &str) -> isize {
    setup(hostname, chroot_dir);

    // run command in isolated environment
    let es: process::ExitStatus;
    if args.len() > 1 {
        es = Command::new(&args[0])
            .args(&args[1..])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("error spawning child process")
            .wait()
            .expect("error waiting for child process to exit");
    } else {
        es = Command::new(&args[0])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("error spawning child process")
            .wait()
            .expect("error waiting for child process to exit");
    }

    unmount_post_run();
    return es.code().unwrap() as isize;
}

fn child_shell(hostname: &str, chroot_dir: &str) -> isize {
    setup(hostname, chroot_dir);

    let mut child_process = Command::new("/bin/bash")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    // TODO: read from parent stdin and write to child stdout

    unmount_post_run();
    return 0;
}

fn set_cgroups() {
    let cgroups = path::Path::new("/sys/fs/cgroup");
    let pids = cgroups.join("pids").join("cfs");

    // permissions 0755
    let mkdir_flags = Mode::S_IRWXU | Mode::S_IRGRP | Mode::S_IWGRP | Mode::S_IROTH | Mode::S_IWOTH;

    // TODO: remove directory once finished
    match unistd::mkdir(&pids, mkdir_flags) {
        Ok(_) => {}
        Err(err) => {
            println!("warning from mkdir: {:?}", err)
        }
    };

    // permissions 0700: owner can read, write, execute
    let cgroup_file_flags = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IXUSR;

    let max_pids = MAX_PIDS;
    write_file(pids.join("pids.max"), max_pids, cgroup_file_flags);

    let notify_on_release = NOTIFY_ON_RELEASE;
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

fn setup(hostname: &str, chroot_dir: &str) {
    set_namespace();
    set_hostname(hostname);
    set_cgroups();
    set_chroot(chroot_dir);
    mount_fs();
}

fn set_namespace() {
    match sched::unshare(sched::CloneFlags::CLONE_NEWNS) {
        Ok(_) => {
            if env::var("DEBUG").is_ok() {
                println!("unshare successful")
            }
        }
        Err(err) => panic!("failed to unshare: {:?}", err),
    };
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
                if in_debug_mode() {
                    println!("success: {}", mr.task_message)
                }
            }
            Err(err) => panic!("failure: error with {}: \n{}", mr.task_message, err),
        }
    }

    pub fn in_debug_mode() -> bool {
        env::var("DEBUG").is_ok()
    }
}

mod filesystem {
    use std::{env, path::Path};
    pub fn join_current_dir(join_path: &str) -> Option<String> {
        let current_dir = env::current_dir().unwrap();
        let joined_path = Path::new(current_dir.as_path()).join(join_path);
        Some(joined_path.to_str().unwrap().to_string())
    }
}
