use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// The file lock elects the owner synchronously, before any Tauri setup or
/// credential/data access. Only that owner binds and cleans the activation socket.
pub struct Instance {
    _lock: File,
    listener: Option<UnixListener>,
    socket: PathBuf,
}

impl Instance {
    pub fn acquire() -> io::Result<Option<Self>> {
        // A short, private per-user directory fits macOS sockaddr_un even when
        // the user's Application Support path or temporary directory is long.
        let uid = unsafe { libc::geteuid() };
        Self::acquire_in(&PathBuf::from(format!("/tmp/aether-desktop-{uid}")))
    }

    fn acquire_in(directory: &Path) -> io::Result<Option<Self>> {
        match fs::create_dir(directory) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "实例目录不属于当前用户",
            ));
        }
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory.join("instance.lock"))?;
        if !lock.metadata()?.is_file() || lock.metadata()?.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "实例锁文件无效",
            ));
        }
        let socket = directory.join("activate.sock");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                match fs::remove_file(&socket) {
                    Ok(()) => (),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                    Err(error) => return Err(error),
                }
                let listener = UnixListener::bind(&socket)?;
                return Ok(Some(Self {
                    _lock: lock,
                    listener: Some(listener),
                    socket,
                }));
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::WouldBlock {
                return Err(error);
            }
            if let Ok(mut stream) = UnixStream::connect(&socket) {
                stream.set_write_timeout(Some(Duration::from_millis(250)))?;
                stream.write_all(b"A")?;
                return Ok(None);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "另一个 Aether 实例正在启动，请稍后重试",
                ));
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn listen(&mut self, activate: impl Fn() + Send + 'static) {
        let Some(listener) = self.listener.take() else {
            return;
        };
        thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                let mut action = [0];
                if stream.read_exact(&mut action).is_ok() && action == *b"A" {
                    activate();
                }
            }
        });
    }

    pub fn cleanup_socket(&self) {
        let _ = fs::remove_file(&self.socket);
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.cleanup_socket();
        // Keep the lock inode in place; unlinking it would permit two owners.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn secondary_activation_and_owner_restart_preserve_the_lock_inode() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let owner = Instance::acquire_in(directory.path()).unwrap().unwrap();
        assert!(Instance::acquire_in(directory.path()).unwrap().is_none());
        let (mut stream, _) = owner.listener.as_ref().unwrap().accept().unwrap();
        let mut action = [0];
        stream.read_exact(&mut action).unwrap();
        assert_eq!(action, *b"A");
        let inode = fs::metadata(directory.path().join("instance.lock"))
            .unwrap()
            .ino();
        drop(owner);
        assert!(!directory.path().join("activate.sock").exists());
        assert!(Instance::acquire_in(directory.path()).unwrap().is_some());
        assert_eq!(
            fs::metadata(directory.path().join("instance.lock"))
                .unwrap()
                .ino(),
            inode
        );
    }

    #[test]
    fn simultaneous_cold_starts_elect_exactly_one_owner() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let barrier = Arc::new(Barrier::new(4));
        let complete = Arc::new(Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let path = directory.path().to_path_buf();
                let barrier = barrier.clone();
                let complete = complete.clone();
                thread::spawn(move || {
                    barrier.wait();
                    let instance = Instance::acquire_in(&path).unwrap();
                    complete.wait();
                    instance.is_some()
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| usize::from(handle.join().unwrap()))
                .sum::<usize>(),
            1
        );
    }
}
