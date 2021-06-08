use nix::{sched, sys::stat::*, unistd};
use std::iter::FromIterator;

use std::process::{self, Command, Stdio};
use std::u8;

const DIR_NAME: &str = "container-fs";
const CONTAINER_HOSTNAME: &str = "cfs-container";
const MAX_PIDS: &str = "20";

pub enum CmdType {
    RUN,
    SHELL,
}

pub struct Container {
    pub args: Vec<String>,
    pub cmd_type: CmdType,
    path: String,
}

impl Container {
    pub fn new(args: &Vec<String>) -> Self {
        if args.len() < 3 {
            panic!("error: you must provide at least two arguments");
        }
        let command = args.get(1).unwrap();
        match &command[..] {
            "run" => Container {
                args: Vec::from_iter(args[2..].iter().cloned()),
                cmd_type: CmdType::RUN,
                path: util::pwd_join(DIR_NAME)
                    .expect(format!("invalid path [{:?}]", DIR_NAME).as_str()),
            },
            "shell" => Container {
                args: vec![String::from("/bin/bash")],
                cmd_type: CmdType::SHELL,
                path: util::pwd_join(DIR_NAME)
                    .expect(format!("invalid path [{:?}]", DIR_NAME).as_str()),
            },
            _ => {
                println!("command not recognized");
                process::exit(1)
            }
        }
    }

    pub fn child_process(
        self: &Self,
        args: &Vec<String>,
        hostname: &str,
        chroot_dir: &str,
    ) -> isize {
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

        proc::unmount_post_run();
        return es.code().unwrap() as isize;
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

            // TODO: make hostname configurable
        let child_process_box =
            Box::new(|| self.child_process(&self.args, CONTAINER_HOSTNAME, self.path.as_str()));
        const STACK_SIZE: usize = 1024 * 1024;
        let mut stack: [u8; STACK_SIZE] = [0; STACK_SIZE];
        match sched::clone(
            child_process_box,
            &mut stack,
            clone_flags,
            Some(nix::sys::signal::SIGCHLD as i32),
        ) {
            Ok(_) => {
                if util::in_debug_mode() {
                    println!("clone succeeded")
                }
            }
            Err(err) => panic!("clone failed: {:?}", err),
        };
    }
}

// run user input command in child process

mod util {
    use nix::{self, sys::stat::*};
    use std::os::unix::prelude::AsRawFd;
    use std::{
        env,
        fs::File,
        io::prelude::*,
        path::{Path, PathBuf},
    };

    pub fn in_debug_mode() -> bool {
        std::env::var("DEBUG").is_ok()
    }

    pub fn pwd_join(join_path: &str) -> Option<String> {
        let current_dir = env::current_dir().unwrap();
        let joined_path = Path::new(current_dir.as_path()).join(join_path);
        Some(joined_path.to_str().unwrap().to_string())
    }

    pub fn write_file(path: PathBuf, content: &str, flags: Mode) {
        let mut file = match File::create(&path.to_str().unwrap()) {
            Ok(file) => file,
            Err(err) => panic!("error creating file at path [{:?}]: {:?}", path, err),
        };

        file.write_all(content.as_bytes())
            .expect("error writing to file");

        fchmodat(
            Some(file.as_raw_fd()),
            &path,
            flags,
            FchmodatFlags::FollowSymlink,
        )
        .expect("failed to set permissions");
    }
}

fn setup(hostname: &str, chroot_dir: &str) {
    namespace::isolated_ns();
    syscall::set_hostname(hostname);
    cgroups::set_cgroups();
    syscall::set_chroot(chroot_dir);
    proc::mount_proc();
}

mod namespace {
    use super::util;
    use nix::sched;
    pub fn isolated_ns() {
        match sched::unshare(sched::CloneFlags::CLONE_NEWNS) {
            Ok(_) => {
                if util::in_debug_mode() {
                    println!("unshare successful")
                }
            }
            Err(err) => panic!("failed to unshare: {:?}", err),
        };
    }
}

mod syscall {
    use nix::unistd;
    pub fn set_hostname(hostname: &str) {
        unistd::sethostname(hostname).expect("set hostname failed");
    }

    pub fn set_chroot(path: &str) {
        unistd::chroot(path).expect("set chroot");
        unistd::chdir("/").expect("change directory to /");
    }
}

mod cgroups {

    use super::*;
    use std::path;

    // TODO: make # PIDS configurable
    pub fn set_cgroups() {
        let cgroups = path::Path::new("/sys/fs/cgroup");
        let pids = cgroups.join("pids").join("cfs");

        // permissions 0755
        let mkdir_flags =
            Mode::S_IRWXU | Mode::S_IRGRP | Mode::S_IWGRP | Mode::S_IROTH | Mode::S_IWOTH;

        // TODO: remove directory once finished
        match unistd::mkdir(&pids, mkdir_flags) {
            Ok(_) => {}
            Err(err) => {
                println!("warning from mkdir: {:?}", err)
            }
        };

        // permissions 0700: owner can read, write, execute
        let cg_flags = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IXUSR;

        let max_pids = MAX_PIDS;
        util::write_file(pids.join("pids.max"), max_pids, cg_flags);

        // remove new cgroup after process finishes
        util::write_file(pids.join("notify_on_release"), "1", cg_flags);

        let cgroup_procs = unistd::getpid().to_string();
        util::write_file(pids.join("cgroup.procs"), cgroup_procs.as_ref(), cg_flags);
    }
}

mod proc {
    use nix::mount;

    pub fn mount_proc() {
        mount::mount(
            Some("proc"),
            "proc",
            Some("proc"),
            mount::MsFlags::empty(),
            Some(""),
        )
        .expect("mount proc");
    }

    pub fn unmount_post_run() {
        mount::umount("/proc").expect("unmounting proc");
    }
}
