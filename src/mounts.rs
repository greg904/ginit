use crate::linux;
use core::convert::{TryFrom, TryInto};

#[derive(Copy, Clone, Debug)]
enum MountParserState {
    BeforeDirectory,
    Directory,
    AfterDirectory,
}

fn read_mounts_from_fd<const N: usize>(fd: u32, out: &mut [u8; N]) -> i32 {
    let mut state = MountParserState::BeforeDirectory;
    let mut cursor = 0;
    loop {
        let mut buf = [0u8; 128];
        let n = unsafe { linux::read(fd, buf.as_mut_ptr(), buf.len()) };
        if n == 0 {
            // EOF
            break;
        } else if n < 0 {
            return n.try_into().unwrap();
        }
        let n = usize::try_from(n).unwrap();

        let mut done = 0;
        loop {
            let remaining = &buf[done..n];
            match state {
                MountParserState::BeforeDirectory => {
                    match remaining.iter().position(|b| *b == b' ') {
                        Some(p) => {
                            state = MountParserState::Directory;

                            done += p + 1;
                            if done >= buf.len() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                MountParserState::Directory => match remaining.iter().position(|b| *b == b' ') {
                    Some(p) => {
                        if cursor + p + 1 >= out.len() {
                            return -linux::ENOMEM;
                        }

                        out[cursor..(cursor + p)].copy_from_slice(&remaining[..p]);
                        cursor += p;

                        out[cursor] = b'\0';
                        cursor += 1;

                        state = MountParserState::AfterDirectory;

                        done += p + 1;
                        if done >= buf.len() {
                            break;
                        }
                    }
                    None => {
                        if cursor + remaining.len() + 1 >= out.len() {
                            return -linux::ENOMEM;
                        }
                        out[cursor..(cursor + remaining.len())].copy_from_slice(&remaining);
                        cursor += remaining.len();

                        out[cursor] = b'\0';
                        cursor += 1;

                        break;
                    }
                },
                MountParserState::AfterDirectory => {
                    match remaining.iter().position(|b| *b == b'\n') {
                        Some(p) => {
                            state = MountParserState::BeforeDirectory;

                            done += p + 1;
                            if done >= buf.len() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    }

    cursor.try_into().unwrap()
}

pub fn read_mounts<const N: usize>(out: &mut [u8; N]) -> i32 {
    let fd = unsafe { linux::open(b"/proc/mounts\0" as *const u8, linux::O_RDONLY, 0) };
    if fd < 0 {
        return fd;
    }
    let fd = linux::Fd(fd.try_into().unwrap());
    read_mounts_from_fd(fd.0, out)
}
