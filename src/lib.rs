use nix::{sched, sys::stat::*, unistd};

use std::process::{self, Command, Stdio};
use std::{os::unix::fs::PermissionsExt, u8};

pub struct Container {
    pub args: Vec<String>,
    chroot_path: String,
    cgroup_name: String,
    hostname: String,
    max_pids: u8,
}

pub struct ContainerBuilder {
    args: Vec<String>,
    chroot_path: String,
    cgroup_name: String,
    hostname: String,
    max_pids: u8,
}

impl ContainerBuilder {
    pub fn new() -> Self {
        ContainerBuilder {
            args: Vec::new(),
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

        mounts::unmount_post_run();
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
    use nix::{self, sys::stat::*, unistd};
    use std::{
        env,
        fs::File,
        io::prelude::*,
        os::unix::prelude::{AsRawFd, PermissionsExt},
        path::{Path, PathBuf},
        process::Command,
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

    pub fn pivot_root(container_fs_path: &str) {
        unistd::chdir(container_fs_path)
            .expect(format!("error changing to directory: {:?}", container_fs_path).as_str());

        let mkdir_flags = Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IROTH | Mode::S_IWOTH;
        let old_root = "oldroot";

        let old_root_string = pwd_join(old_root).unwrap();

        // avoid this memory alloc if possible?
        let old_root_path = Path::new(&old_root_string);
        if old_root_path.exists() {
            let mut metadata = std::fs::metadata(old_root_path).unwrap().permissions();
            metadata.set_mode(0o666);
            std::fs::set_permissions(old_root_path, metadata).ok();

            // remove everything first with fs::remove_dir_all if it exists
            let _res = std::fs::remove_dir_all(old_root);
            println!("remove_dir_all result: {:?}", _res);
        }

        let _res = unistd::mkdir(old_root, mkdir_flags);

        // syscall::pivot_root(".", old_root);
        match unistd::pivot_root(".", old_root) {
            Ok(_) => (),
            Err(error) => println!("Error pivoting root: {}", error),
        }

        unistd::chdir("/").expect("unable to change into root directory");
    }

    #[allow(dead_code)]
    pub fn make_temp_fs(temp_dir_path: &str, fs_src: &str) {
        // permissions 0775
        let mkdir_flags = Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IROTH | Mode::S_IWOTH;
        let _res = unistd::mkdir(temp_dir_path, mkdir_flags);

        let _exit_status = Command::new("/bin/cp")
            .args(vec!["-r", fs_src, temp_dir_path])
            .spawn()
            .expect("error spawning cp command")
            .wait()
            .expect("error waiting for cp process to exit");
    }
}

fn setup(container: &Container) {
    namespace::isolated_ns();
    syscall::set_hostname(container.hostname.as_str());
    mounts::bind_mount(container.chroot_path.as_ref());
    cgroups::set_cgroups(container).unwrap();

    // let last_dir = path::Path::new(container.chroot_path.as_str()).file_stem().unwrap();
    // let container_fs_path = path::Path::new(container_fs_path).join(last_dir.to_str().unwrap());
    util::pivot_root(container.chroot_path.as_ref());

    syscall::set_chroot("/");
    mounts::mount_proc();
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

    // #[allow(dead_code)]
    // pub fn pivot_root(new_root: &str, old_root: &str) {
    // }
}

mod cgroups {

    use super::*;
    use std::{fs, path::{self, PathBuf}};

    #[allow(dead_code)]
    pub fn rm_cgroup_dir(container: &Container) {
        let cg_path = path::Path::new("/sys/fs/cgroup/pids");
        let cg_path = cg_path.join(container.cgroup_name.clone());

        println!("removing directory: {:?}", cg_path);
        match fs::remove_dir_all(cg_path) {
            Ok(_) => (),
            Err(err) => println!("error removing directory: {:?}", err),
        };
    }

    fn set_dir_permissions(dir_path : &PathBuf) -> std::io::Result<()> {
            // shamelessly copied from https://doc.rust-lang.org/std/fs/fn.read_dir.html
            for entry in std::fs::read_dir(&dir_path)? {
                let entry = entry?;
                let path = entry.path();
                println!("checking {:?}", path);
                if path.is_file() {
                    let mut perm = std::fs::metadata(&path).unwrap().permissions();
                    println!("setting permissions for {:?}", path);
                    perm.set_mode(0o666);
                    std::fs::set_permissions(path, perm).ok();
                }
            }
            Ok(())
    }

    // TODO: make # PIDS configurable
    pub fn set_cgroups(container: &Container) -> std::io::Result<()> {
        let cgroups = path::Path::new("/sys/fs/cgroup");
        let container_cg_path = cgroups.join("pids").join(container.cgroup_name.clone());

        // permissions 0775
        let mkdir_flags = Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IROTH | Mode::S_IWOTH;

        // TODO: Move to a separate function!!
        // remove everything before mkdir if it exists
        println!("checking path: {:?}", container_cg_path);
        if container_cg_path.exists() && container_cg_path.is_dir() {
            
            set_dir_permissions(&container_cg_path)?;

            let _res = std::fs::remove_dir_all(&container_cg_path);
            println!("remove_dir_all result: {:?}", _res);
        }

        // TODO: instead consider setting permissions with std::fs functions first
        match unistd::mkdir(&container_cg_path, mkdir_flags) {
            Ok(_) => {}
            Err(err) => println!("warning from mkdir: {:?}", err),
        };

        // permissions 0700: owner can read, write, execute
        let cg_flags = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IXUSR;

        let max_pids = container.max_pids.to_string();
        util::write_file(
            container_cg_path.join("pids.max"),
            max_pids.as_ref(),
            cg_flags,
        );

        // remove new cgroup after process finishes
        util::write_file(container_cg_path.join("notify_on_release"), "1", cg_flags);

        let cgroup_procs = unistd::getpid().to_string();
        util::write_file(
            container_cg_path.join("cgroup.procs"),
            cgroup_procs.as_ref(),
            cg_flags,
        );

        Ok(())
    }
}

mod mounts {
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

    pub fn bind_mount(root_fs_path: &str) {
        let _er = mount::mount(
            Some(root_fs_path),
            root_fs_path,
            None::<&str>,
            mount::MsFlags::MS_BIND | mount::MsFlags::MS_REC,
            None::<&str>,
        );
    }

    pub fn unmount_post_run() {
        mount::umount("/proc").expect("unmounting proc");
    }
}
