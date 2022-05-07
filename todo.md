# Right now

* Clean up init.
* Find a way to put all important configuration files (such as /etc and
  ~/.config) into the the `bubble` directory.

# Ideas for later

Note: some changes are to be done when the system is already working.
Also, the system should stay simple like old distros and BSD. Complexity
introduces throughout, latency, bugs and friction of development.

* Think about using litterate programming to write the programs.
* http://www.pathsensitive.com/2021/03/developer-tools-can-be-magic-instead.html
* Think about other ways to improve comprehension of the code and the high
  level architecture.

* Purgeable memory
* Inline/static link libc
* Custom libc and get rid of TLS errno and other bad stuff
* Get rid of process initialization code
* Reuse parts of initial process stack when no longer used
* Real-time/tuned scheduling for every process
* Use "packed" filesystems for read-only stuff like packages because they won't
  fragment and might be faster. Examples: squashfs and erofs
* Init memory unmapped or made read-only after init (userspace as well as kernel)
* More priviledge separation
* Syscall origin check
* GCC default stack to not executable
* Zero copy IPC with shared memory
* No fork exec (vfork, spawn, clone)
* Tests
* Boot logo/animation
* Disable `CONFIG_VT`
