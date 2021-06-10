use nix::{sched, sys::stat::*, unistd};

use std::process::{self, Command, Stdio};
use std::u8;

pub enum CmdType {
    RUN,
    SHELL,
}

pub struct Container {
    pub args: Vec<String>,
    pub cmd_type: CmdType,
    chroot_path: String,
    cgroup_name: String,
    hostname: String,
    max_pids: u8,
}

pub struct ContainerBuilder {
    args: Vec<String>,
    cmd_type: CmdType,
    chroot_path: String,
    cgroup_name: String,
    hostname: String,
    max_pids: u8,
}

impl ContainerBuilder {
    pub fn new() -> Self {
        ContainerBuilder {
            args: Vec::new(),
            cmd_type: CmdType::RUN,
            chroot_path: String::new(),
            cgroup_name: String::new(),
            hostname: String::new(),
            max_pids: 0,
        }
    }
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
    pub fn chroot_path(mut self, path: String) -> Self {
        self.chroot_path = path;
        self
    }
    pub fn cmd_type(mut self, cmdtype: CmdType) -> Self {
        self.cmd_type = cmdtype;
        self
    }
    pub fn max_pids(mut self, max_pids: u8) -> Self {
        self.max_pids = max_pids;
        self
    }
    pub fn cgroup_name(mut self, cgroup_name: &str) -> Self {
        self.cgroup_name = String::from(cgroup_name);
        self
    }
    pub fn hostname(mut self, hostname: &str) -> Self {
        self.hostname = String::from(hostname);
        self
    }
    pub fn create(self) -> Container {
        Container {
            args: self.args,
            cmd_type: self.cmd_type,
            chroot_path: self.chroot_path,
            cgroup_name: self.cgroup_name,
            hostname: self.hostname,
            max_pids: self.max_pids,
        }
    }
}

impl Container {
    pub fn child_process(self: &Self) -> isize {
        setup(&self);

        // make child process output apparent
        println!();

        // run command in isolated environment
        let es: process::ExitStatus;
        if self.args.len() > 1 {
            es = Command::new(&self.args[0])
                .args(&self.args[1..])
                .stdin(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .spawn()
                .expect("error spawning child process")
                .wait()
                .expect("error waiting for child process to exit");
        } else {
            es = Command::new(&self.args[0])
                .stdin(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .spawn()
                .expect("error spawning child process")
                .wait()
                .expect("error waiting for child process to exit");
        }

        proc::unmount_post_run();
        return es.code().unwrap() as isize;
    }

    pub fn run(self: Self) {

        let clone_flags = sched::CloneFlags::CLONE_NEWUTS
            | sched::CloneFlags::CLONE_NEWPID
            | sched::CloneFlags::CLONE_NEWNS;

        let child_process_box = Box::new(|| self.child_process());
        const STACK_SIZE: usize = 1024 * 1024;
        let mut stack: [u8; STACK_SIZE] = [0; STACK_SIZE];
        sched::clone(
            child_process_box,
            &mut stack,
            clone_flags,
            Some(nix::sys::signal::SIGCHLD as i32),
        )
        .expect("clone failed");
    }
}

// run user input command in child process

pub mod util {
    use nix::{self, sys::stat::*};
    use std::{
        env,
        fs::File,
        io::prelude::*,
        os::unix::prelude::AsRawFd,
        path::{Path, PathBuf},
    };

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

fn setup(container: &Container) {
    namespace::isolated_ns();
    syscall::set_hostname(container.hostname.as_str());
    cgroups::set_cgroups(container);
    syscall::set_chroot(container.chroot_path.as_ref());
    proc::mount_proc();
}

mod namespace {
    use nix::sched;
    pub fn isolated_ns() {
        sched::unshare(sched::CloneFlags::CLONE_NEWNS).expect("failed to unshare");
    }
}

mod syscall {
    use nix::unistd;
    pub fn set_hostname(hostname: &str) {
        unistd::sethostname(hostname).expect("set hostname failed");
    }

    pub fn set_chroot(path: &str) {
        // TODO: chroot may not be easy enough to "undo" - look into pivot_root instead
        unistd::chroot(path).expect("set chroot");
        unistd::chdir("/").expect("change directory to /");
    }
}

mod cgroups {

    use super::*;
    use std::path;

    #[allow(dead_code)]
    pub fn rm_cgroup_dir(container: &Container) {
        let cg_path = path::Path::new("/sys/fs/cgroup/pids");
        let cg_path = cg_path.join(container.cgroup_name.clone());
        println!("removing directory: {:?}", cg_path);
        match std::fs::remove_dir_all(cg_path){
               Ok(_) => (),
               Err(err) => println!("error removing directory: {:?}", err),
        };
    }

    // TODO: make # PIDS configurable
    pub fn set_cgroups(container: &Container) {
        let cgroups = path::Path::new("/sys/fs/cgroup");
        let pids = cgroups.join("pids").join(container.cgroup_name.clone());

        // permissions 0775
        let mkdir_flags =
            Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IROTH | Mode::S_IWOTH;

        // TODO: remove directory once finished
        match unistd::mkdir(&pids, mkdir_flags) {
            Ok(_) => {}
            Err(err) => println!("warning from mkdir: {:?}", err),
        };

        // permissions 0700: owner can read, write, execute
        let cg_flags = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IXUSR;

        let max_pids = container.max_pids.to_string();
        util::write_file(pids.join("pids.max"), max_pids.as_ref(), cg_flags);

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
