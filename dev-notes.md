# dev-notes

## what?

Let's go over some of the basic Linux components that
help us create containers.

### container primitives:

1. Linux Kernel User & System Space
2. Syscalls and Capabilities
3. Cgroups
4. Namespaces
5. EIAF (Everything Is A File) - description of the Unix based Filesystem

### linux kernel user & system space

```
User Space                        Kernel Space
[process]------(system calls)---->[RAM]
                                  [Disk]
```

Your processes usually will run in User Space, where to actually
do anything you have to make system calls to the Kernel Space.
The Kernel Space is where we have control over system memory and
disk access for all programs running on your host machine. It's the
highest level of privilege on your Linux machine.

Hierarchy of OS layers:

```
User Programs --> Library/Interpreter --> System Calls --> Kernel Space
```

Apps make requests to kernel level functions, triggering an interrupt
that's sent to the processor to stop and handle that particular
request by doing whatever context switching is required and accessing
whatever resources are needed.


### syscalls and capabilities

A small part of the kernel space exposed in an API to programs. Syscalls
are OS specific: there are different sets of syscalls available on
Linux/MacOS/Windows operating system kernels.

### cgroups

(from `man cgroup 7`)

Cgroups are a Linux kernel feature which allow processes to be organized into hierarchical
groups whose usage of various types of resources can then be limited and monitored.

Cgroups say what system resources you can use. They allow groups or processes to
be restricted to a limited amount of resources. You can manage many different
cgroups for things such as cpu shares, memory, number of processes (pids), block
devices, devices, etc.


### namespaces

Namespaces restrict what processes can "see" on a system. They make containers look
like they're operating in an isolated environment.

Namespaces make processes within a container only see themselves and not the processes on the host system.

There are the following namespace types:

```
       Namespace Flag            Page                  Isolates
       Cgroup    CLONE_NEWCGROUP cgroup_namespaces(7)  Cgroup root directory
       IPC       CLONE_NEWIPC    ipc_namespaces(7)     System V IPC,
                                                       POSIX message queues
       Network   CLONE_NEWNET    network_namespaces(7) Network devices,
                                                       stacks, ports, etc.
       Mount     CLONE_NEWNS     mount_namespaces(7)   Mount points
       PID       CLONE_NEWPID    pid_namespaces(7)     Process IDs
       User      CLONE_NEWUSER   user_namespaces(7)    User and group IDs
       UTS       CLONE_NEWUTS    uts_namespaces(7)     Hostname and NIS
                                                       domain name
```

The namespaces API includes:

* `clone(2)`: creates a new process, you can create a new namespace with a `CLONE_NEW*` flag
* `setns(2)`: changes namespace of the calling process to an existing namespace
* `unshare(2)`: moves calling process into a new namespace
* `ioctl(2)`: can discover information about namespaces

### the linux filesystem


See `man namespaces 7` for more details.


## development notes

Using `pivot_root` syscall
* helpful Stack Exchange answer and associated code snippet: https://unix.stackexchange.com/a/155824/446190

This sequence of commands will setup a new filesystem root to run the container in `/ramroot`.

The goal of executing these commands is to have a filesystem that can be mounted and unmounted at will.

```bash
mkdir /newroot
mount -n -t tmpfs -o size=500M none /newroot
cd container-fs # (containing the root filesystem contents)
find . -depth -xdev -print | cpio -pd --quiet /newroot
cd /newroot
mkdir oldroot
# possibly need to run `unshare -m` before this
pivot_root . oldroot
exec chroot . bin/sh
umount oldroot
```

---

linux commands to "make" containers

Mount filesystem so `pivot_root` succeeds:

```bash
# --bind mount remounts part of the file hierarchy somewhere else
mount --bind /containers/tupperware /containers/tupperware

# --move moves a mounted fs to another place atomically
mount --move /containers/tupperware /containers/tupperware
```

To unmount all devices:

```bash
umount -a
```

To unmount the "oldroot"

```bash
umount -l /oldroot/
```

Remount proc:

```bash
mount -t proc none /proc
```