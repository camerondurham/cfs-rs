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
mount -n -t tmpfs -o size=500M,rw none /newroot
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

Commands blindly copied from a 2015 talk at Docker Con: 
https://www.youtube.com/watch?v=sK5i-N34im8

```bash
# unshare these namespaces - isolate the container
unshare --mount --uts --ipc --net --pid --fork bash

# set the new hostname
hostname tupperware

# start a shell into the isolated environment
exec bash

# --bind mount remounts part of the file hierarchy somewhere else
mount --bind /containers/tupperware /containers/tupperware

# --move moves a mounted fs to another place atomically
mount --move /containers/tupperware /containers

# remount so it's writable
mount -o remount,rw /containers

# go to the newly mounted filesystem
cd /containers
mkdir oldroot
pivot_root . oldroot
cd /

# remount so we can see proceses
mount -t proc none /proc

# To unmount all devices:
umount -a

# verify host system mounts are not there anymore
mount

# Remount proc:
mount -t proc none /proc

# To unmount the "oldroot"
umount -l /oldroot/

# check mounts again
mount

# do stuff in the container (not that you won't be able to access the internet yet!)

# now get a "real shell" into the container
cd /
exec chroot / sh
```


### commands that should be made into a shell script

```bash
# create workspace
mkdir /container
cp -r /home/container-fs/* /container

# unshare these namespaces - isolate the container
unshare --mount --uts --ipc --net --pid --fork bash

# set the new hostname
hostname tupperware

# start a shell in the new "hostname"
exec bash

mount --bind /container /container

cd /container
mkdir oldroot
pivot_root . oldroot
cd /



# mount so we can see processes
mount -t proc none /proc

# unmount everything
umount -a
umount -l /oldroot

# check it out
mount

# make it writeable
mount -o remount,rw /

cd /
exec chroot / sh
```

### bash session from running the above commands:

```bash
cam@box:~/projects/cfs-rs$ make run
docker run --privileged -it cfs-rs:v1
root@580801bb3cf5:/home# mkdir /container
root@580801bb3cf5:/home# cp -r /home/container-fs/* /container/
root@580801bb3cf5:/home# unshare --mount --uts --ipc --net --pid --fork bash
root@580801bb3cf5:/home# hostname tupperware
root@580801bb3cf5:/home# exec bash
root@tupperware:/home# mount --bind /container/ /container/
root@tupperware:/home# cd /container/
root@tupperware:/container# mount --bind /container/ /container/^C
root@tupperware:/container# touch test
root@tupperware:/container# ls
CONTAINER_ROOT_DIR  check  home   lib64   mnt   root  srv   tmp
bin                 dev    lib    libx32  opt   run   sys   usr
boot                etc    lib32  media   proc  sbin  test  var
root@tupperware:/container# rm test
root@tupperware:/container# mkdir oldroot
root@tupperware:/container# pivot_root . oldroot/
root@tupperware:/container# cd /
root@tupperware:/# ls
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  tmp
bin                 dev    lib    libx32  oldroot  root  srv   usr
boot                etc    lib32  media   opt      run   sys   var
root@tupperware:/# touch test
root@tupperware:/# mount -t proc none /proc
root@tupperware:/# umount -a 
umount: /oldroot/dev: target is busy.
umount: /oldroot: target is busy.
root@tupperware:/# mount -l /oldroot
mount: /oldroot: can\'t find in /etc/fstab.
root@tupperware:/# ls
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  test  var
bin                 dev    lib    libx32  oldroot  root  srv   tmp
boot                etc    lib32  media   opt      run   sys   usr
root@tupperware:/# umount -l /oldroot
root@tupperware:/# ls
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  test  var
bin                 dev    lib    libx32  oldroot  root  srv   tmp
boot                etc    lib32  media   opt      run   sys   usr
root@tupperware:/# mount -o remount,rw /
root@tupperware:/# ls
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  test  var
bin                 dev    lib    libx32  oldroot  root  srv   tmp
boot                etc    lib32  media   opt      run   sys   usr
root@tupperware:/# ls /
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  test  var
bin                 dev    lib    libx32  oldroot  root  srv   tmp
boot                etc    lib32  media   opt      run   sys   usr
root@tupperware:/# ls /
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  test  var
bin                 dev    lib    libx32  oldroot  root  srv   tmp
boot                etc    lib32  media   opt      run   sys   usr
root@tupperware:/# touch test
root@tupperware:/# cd /
root@tupperware:/# ls
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  test  var
bin                 dev    lib    libx32  oldroot  root  srv   tmp
boot                etc    lib32  media   opt      run   sys   usr
root@tupperware:/# exec chroot / sh
# ls
CONTAINER_ROOT_DIR  check  home   lib64   mnt      proc  sbin  test  var
bin                 dev    lib    libx32  oldroot  root  srv   tmp
boot                etc    lib32  media   opt      run   sys   usr
# touch test
# mount
overlay on / type overlay (rw,relatime,lowerdir=/var/lib/docker/overlay2/l/KLXXAWEUM45TIMFIJLFCY4QXCS:/var/lib/docker/overlay2/l/SLM5OTDMR4VCBDJYXH5WT2QOPA:/var/lib/docker/overlay2/l/A55GRKNQXDYWUHJXQONPJLEJFE:/var/lib/docker/overlay2/l/N5MW27QCGEYCWLBX2VCCINNVXB:/var/lib/docker/overlay2/l/FUKLNYGSSYXJ7IG4IWHQ6UIJJM:/var/lib/docker/overlay2/l/WEL7JYBL7YIHU25BE2QIDTKKNN:/var/lib/docker/overlay2/l/S2Z4KOIKOXHPLKIKL2HQWXGR77:/var/lib/docker/overlay2/l/7BA7AKGZXVQJQMKFD7QPVBT7N5:/var/lib/docker/overlay2/l/ETF7NVENOK7KEMQ2ITVWXU4URJ:/var/lib/docker/overlay2/l/VBOZYMOQRX6OTSGU45JLRBKD6V:/var/lib/docker/overlay2/l/OPKG3PJ5K3USYVADCFXMBJVYA7:/var/lib/docker/overlay2/l/F4GG4YRTMYVPIWO32ZQCZTKAG6,upperdir=/var/lib/docker/overlay2/6d8483209333b20c726fa78ad7480e81029fb4e59971d4b6bc5ec0886680d79d/diff,workdir=/var/lib/docker/overlay2/6d8483209333b20c726fa78ad7480e81029fb4e59971d4b6bc5ec0886680d79d/work)
none on /proc type proc (rw,relatime)
# ps
  PID TTY          TIME CMD
    1 ?        00:00:00 sh
   26 ?        00:00:00 ps
# 
```