#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]
#[allow(warnings)]
mod lfs {
    include!("bindings.rs");
}
use crate::{guard, lock::Lock};
#[deny(warnings)]
use either::{Either, Left, Right};
use id_pool::IdPool;
use js_sys::Reflect;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::sync::Once;
use wasm_bindgen::{JsError, JsValue};

/// multiple workers may be competing to initially mount the filesystem, this
/// lock ensures that only one worker does the work and the others wait for it.
static mut LFS_SYS_INIT_LOCK: Lazy<Lock> = Lazy::new(|| Lock::new().unwrap());
/// A "once" utility to actually mount the filesystem.
static INIT: Once = Once::new();
/// This lock is passed to LittleFS internals, do not use.
static mut LFS_SYS_DISK_LOCK: Lazy<Lock> = Lazy::new(|| Lock::new().unwrap());
/// LittleFS calls this function to lock the disk.
unsafe extern "C" fn lock() {
    LFS_SYS_DISK_LOCK.acquire();
}
/// LittleFS calls this function to unlock the disk.
unsafe extern "C" fn unlock() {
    LFS_SYS_DISK_LOCK.release();
}

pub unsafe fn lfs_locked() -> bool {
    LFS_SYS_DISK_LOCK.held()
}

pub unsafe fn lfs_diag() {
    web_sys::console::log_1(
        &format!("[WASABIO:LFS] LFS_SYS_OPEN_FDS: {:?}", LFS_SYS_OPEN_FDS).into(),
    );
    web_sys::console::log_1(
        &format!("[WASABIO:LFS] LFS_SYS_FD_POOL: {:?}", LFS_SYS_FD_POOL).into(),
    );
    web_sys::console::log_1(
        &format!("[WASABIO:LFS] LFS_SYS_INO_POOL: {:?}", LFS_SYS_INO_POOL).into(),
    );
    web_sys::console::log_1(
        &format!("[WASABIO:LFS] LFS_SYS_HARD_LINKS: {:?}", LFS_SYS_HARD_LINKS).into(),
    );
}

pub unsafe fn lfs_reset() {
    LFS_SYS_DISK_LOCK = Lazy::new(|| Lock::new().unwrap());
    LFS_SYS_INIT_LOCK = Lazy::new(|| Lock::new().unwrap());
    LFS_SYS_OPEN_FDS = Lazy::new(|| HashMap::new());
    LFS_SYS_FD_POOL = Lazy::new(|| IdPool::new());
}

// generates a `const BUILD_TIME: &str`
build_timestamp::build_time!("%s");

pub const S_IFMT: u32 = lfs::lfs_sys_file_type_LFS_SYS_S_IFMT;
pub const S_IFDIR: u32 = lfs::lfs_sys_file_type_LFS_SYS_S_IFDIR;
pub const S_IFREG: u32 = lfs::lfs_sys_file_type_LFS_SYS_S_IFREG;
pub const S_IFLNK: u32 = lfs::lfs_sys_file_type_LFS_SYS_S_IFLNK;

// Read permission for the owner
// pub const S_IRUSR: u32 = 0o400;
// Write permission for the owner
pub const S_IWUSR: u32 = 0o200;
// Execute permission for the owner
// pub const S_IXUSR: u32 = 0o100;
// Read permission for the group
// pub const S_IRGRP: u32 = 0o040;
// Write permission for the group
pub const S_IWGRP: u32 = 0o020;
// Execute permission for the group
// pub const S_IXGRP: u32 = 0o010;
// Read permission for others
// pub const S_IROTH: u32 = 0o004;
// Write permission for others
pub const S_IWOTH: u32 = 0o002;
// Execute permission for others
// pub const S_IXOTH: u32 = 0o001;

pub const DEFAULT_PERM_DIR: i32 = 0o777;
pub const DEFAULT_PERM_FILE: i32 = 0o666;

fn get_default_permissions(is_dir: bool) -> i32 {
    if is_dir {
        DEFAULT_PERM_DIR
    } else {
        DEFAULT_PERM_FILE
    }
}

/// note: once compiled to SharedArrayBuffer, this is AtomicBool essentially.
static mut INITIALIZED: bool = false;

/// Returns a pointer to the LittleFS filesystem object.
/// This is thread and worker safe.
fn disk() -> *mut lfs::lfs_t {
    if !unsafe { INITIALIZED } {
        guard!(LFS_SYS_INIT_LOCK);
        if unsafe { !INITIALIZED } {
            INIT.call_once(|| unsafe {
                let sizeMB = 256; // 256MB of in-memory storage. Cannot be resized.
                lfs::lfs_sys_mount(sizeMB, Some(lock), Some(unlock));
                let root = "/";
                let c_root = CString::new(root).unwrap();
                lfs::lfs_sys_attr_reset(c_root.as_ptr());
                let q = AttrQueryHandle::new(root);
                (*q.0).mode = S_IFDIR as i32 | DEFAULT_PERM_DIR;
                (*q.0).uid = 0;
                (*q.0).gid = 0;
                lfs::lfs_sys_attr_patch(c_root.as_ptr(), q.0);
                Touch::birthtime(root, Some(BUILD_TIME.parse::<f64>().unwrap()));
                console_error_panic_hook::set_once();
                INITIALIZED = true;
            });
        }
    }
    unsafe { lfs::lfs_sys_disk() }
}

struct Touch {/* utility to change attributes */}

impl Touch {
    /// only dependency we have on js_sys is for Date::now()
    pub fn time(t: Option<f64>) -> f64 {
        t.unwrap_or(js_sys::Date::now())
    }
    /// Updates all timing info for a given path.
    pub fn birthtime(path: &str, t: Option<f64>) {
        let t = Self::time(t);
        let c_path = CString::new(path).unwrap();
        let q = AttrQueryHandle::new(path);
        unsafe {
            // when birthtime is set, all other times are set to the same value.
            (*q.0).birthtime = t;
            (*q.0).mtime = t;
            (*q.0).atime = t;
            (*q.0).ctime = t;
            lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
        }
    }
    /// Updates the mtime for a given path (the time the file was last modified)
    /// The mtime refers to the last time when a file’s content was modified.
    pub fn mtime(path: &str, t: Option<f64>) {
        let c_path = CString::new(path).unwrap();
        let q = AttrQueryHandle::new(path);
        unsafe {
            (*q.0).mtime = Self::time(t);
            lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
        }
    }
    /// Updates the atime for a given path (the time the file was last accessed)
    /// The atime indicates the last time when a file was read, including reading
    /// by users directly or through commands or scripts.
    pub fn atime(path: &str, t: Option<f64>) {
        let c_path = CString::new(path).unwrap();
        let q = AttrQueryHandle::new(path);
        unsafe {
            (*q.0).atime = Self::time(t);
            lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
        }
    }
    /// Updates the ctime for a given path (the time the file was last changed)
    /// The ctime refers to the last time when a file’s metadata, such as its
    /// ownership, location, file type and permission settings, was changed.
    pub fn ctime(path: &str, t: Option<f64>) {
        let c_path = CString::new(path).unwrap();
        let q = AttrQueryHandle::new(path);
        unsafe {
            (*q.0).ctime = Self::time(t);
            lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
        }
    }
}

const O_RDONLY: u32 = lfs::lfs_open_flags_LFS_O_RDONLY;
const O_WRONLY: u32 = lfs::lfs_open_flags_LFS_O_WRONLY;
const O_RDWR: u32 = lfs::lfs_open_flags_LFS_O_RDWR;
const O_CREAT: u32 = lfs::lfs_open_flags_LFS_O_CREAT;
const O_TRUNC: u32 = lfs::lfs_open_flags_LFS_O_TRUNC;
const O_APPEND: u32 = lfs::lfs_open_flags_LFS_O_APPEND;
const O_EXCL: u32 = lfs::lfs_open_flags_LFS_O_EXCL;

/// Goes from "rwx" that Node uses to i32 that LittleFS uses.
/// See: https://nodejs.org/api/fs.html#file-system-flags
/// See: https://github.com/streamich/memfs/blob/48f6fbcdce51f62d005648a4beb2eece52d0c1f8/src/volume.ts#L155
pub fn fs_flag_node_to_lfs(flags: Option<&str>) -> i32 {
    let flags = flags.unwrap_or("r");
    let flags = match flags {
        // Open file for reading. An exception occurs if the file does not exist.
        "r" | "rs" | "sr" => O_RDONLY,
        // Open file for reading and writing. An exception occurs if the file does not exist.
        "r+" | "rs+" | "sr+" => O_RDWR,
        // Open file for writing. The file is created (if it does not exist) or truncated (if it exists).
        "w" => O_WRONLY | O_CREAT | O_TRUNC,
        // Like 'w' but fails if path exists.
        "wx" | "xw" => O_WRONLY | O_CREAT | O_TRUNC | O_EXCL,
        // Open file for reading and writing. The file is created (if it does not exist) or truncated (if it exists).
        "w+" => O_RDWR | O_CREAT | O_TRUNC,
        // Like 'w+' but fails if path exists.
        "wx+" | "xw+" => O_RDWR | O_CREAT | O_TRUNC | O_EXCL,
        // Open file for appending. The file is created if it does not exist.
        "a" => O_WRONLY | O_APPEND | O_CREAT,
        // Like 'a' but fails if path exists.
        "ax" | "xa" => O_WRONLY | O_APPEND | O_CREAT | O_EXCL,
        // Open file for reading and appending. The file is created if it does not exist.
        "a+" => O_RDWR | O_APPEND | O_CREAT,
        // Like 'a+' but fails if path exists.
        "ax+" | "xa+" => O_RDWR | O_APPEND | O_CREAT | O_EXCL,
        _ => O_RDONLY, // Default to read only.
    };
    flags as i32
}

#[derive(Debug)]
struct InfoHandle(*mut lfs::lfs_info);

impl InfoHandle {
    fn new() -> Self {
        Self(unsafe { lfs::lfs_sys_info_new() })
    }
}

impl Drop for InfoHandle {
    fn drop(&mut self) {
        unsafe { lfs::lfs_sys_info_free(self.0) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeStats {
    pub dev: f64,
    pub ino: f64,
    pub mode: u16,
    pub nlink: i32,
    pub uid: i32,
    pub gid: i32,
    pub rdev: usize,
    pub size: usize,
    pub blksize: usize,
    pub blocks: usize,
    pub atimeMs: f64,
    pub mtimeMs: f64,
    pub ctimeMs: f64,
    pub birthtimeMs: f64,
}

/// Universal handle to either a directory or a file.
/// "Left" type is a file and "Right" type is a directory always.
type Handle = Either<FileHandle, DirHandle>;

static mut LFS_SYS_FD_POOL: Lazy<IdPool> = Lazy::new(|| IdPool::new());
static mut LFS_SYS_INO_POOL: Lazy<IdPool> = Lazy::new(|| IdPool::new());
static mut LFS_SYS_OPEN_FDS: Lazy<HashMap<usize, Handle>> = Lazy::new(|| HashMap::new());
static mut LFS_SYS_HARD_LINKS: Lazy<HashMap<String, Vec<String>>> = Lazy::new(|| HashMap::new());

/// Looks up a file descriptor in the global map of open files.
/// This is thread and worker safe. Locks the FD lock.
fn lookup_by_fd(fd: usize) -> Option<&'static mut Handle> {
    if fd < 3 {
        return None;
    }
    unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }
}

/// Looks up a file descriptor in the global map of open files.
/// This is thread and worker safe. Locks the FD lock.
fn lookup_by_path(path: &str) -> Option<&'static mut Handle> {
    unsafe {
        LFS_SYS_OPEN_FDS
            .iter_mut()
            .find(|(_, handle)| match handle {
                Either::Left(file) => file.path == path,
                Either::Right(dir) => dir.path == path,
            })
            .map(|(_, handle)| handle)
    }
}

pub fn is_directory(path: &str) -> bool {
    if path == "/" {
        return true;
    }
    let disk = disk();
    let path = CString::new(path.clone()).unwrap();
    let info = InfoHandle::new();
    let res = unsafe { lfs::lfs_stat(disk, path.as_ptr(), info.0) };
    res == lfs::lfs_error_LFS_ERR_OK
        && unsafe { (*info.0).type_ } == lfs::lfs_type_LFS_TYPE_DIR as u8
}

pub fn is_file(path: &str) -> bool {
    if path == "/" || !exists_sync_no_follow(path) {
        return false;
    }
    !is_directory(path)
}

fn is_symlink(path: &str) -> bool {
    if path == "/" || !exists_sync_no_follow(path) {
        return false;
    }
    let q = AttrQueryHandle::new(path);
    unsafe { (*q.0).symlink }
}

fn is_link(path: &str) -> bool {
    if path == "/" || !exists_sync_no_follow(path) {
        return false;
    }
    let q = AttrQueryHandle::new(path);
    unsafe { (*q.0).link }
}

fn is_open(path: &str) -> bool {
    lookup_by_path(path).is_some()
        || if unsafe { LFS_SYS_HARD_LINKS.contains_key(path) } {
            unsafe {
                LFS_SYS_HARD_LINKS.get(path).unwrap().iter().any(|link| {
                    LFS_SYS_OPEN_FDS.iter().any(|(_, handle)| match handle {
                        Either::Left(file) => file.path == link.clone(),
                        Either::Right(dir) => dir.path == link.clone(),
                    })
                })
            }
        } else {
            false
        }
        || if is_directory(path) {
            unsafe {
                LFS_SYS_OPEN_FDS.iter().any(|(_, handle)| match handle {
                    Either::Left(file) => file.path.starts_with(path),
                    Either::Right(dir) => dir.path.starts_with(path),
                })
            }
        } else {
            false
        }
}

#[derive(Debug)]
struct StatHandle(*mut lfs::lfs_sys_statvfs_t);

impl StatHandle {
    fn new() -> Self {
        let ptr = unsafe { lfs::lfs_sys_statvfs_new() };
        Self { 0: ptr }
    }
}

impl Drop for StatHandle {
    fn drop(&mut self) {
        unsafe {
            lfs::lfs_sys_statvfs_free(self.0);
        }
    }
}

#[derive(Debug)]
pub struct StatFs {
    pub bsize: usize,
    pub blocks: usize,
    pub bfree: usize,
    pub bavail: usize,
    pub files: usize,
    pub ffree: usize,
    pub dirs: usize,
    pub json: Option<String>,
}

#[derive(Debug)]
struct AttrQueryHandle(*mut lfs::lfs_sys_query_t);

impl AttrQueryHandle {
    fn new(path: &str) -> Self {
        let path = CString::new(path).unwrap();
        let ptr = unsafe { lfs::lfs_sys_attr_query_new(path.as_ptr()) };
        Self { 0: ptr }
    }
}

impl Drop for AttrQueryHandle {
    fn drop(&mut self) {
        unsafe {
            lfs::lfs_sys_attr_query_free(self.0);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dirent {
    pub name: String,
    pub path: String,
    pub file: bool,
    pub symlink: bool,
}

impl Dirent {
    fn new(path: &str) -> Self {
        let file = is_file(path);
        let symlink = is_symlink(path);
        let name = path_basename(path);
        let path = path_dirname(path);
        Self {
            name,
            path,
            file,
            symlink,
        }
    }
}

#[derive(Debug)]
struct FileHandle {
    file: *mut lfs::lfs_file_t,
    pub path: String,
    pub fd: usize,
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        let disk = disk();
        unsafe {
            if LFS_SYS_OPEN_FDS.contains_key(&(self.fd)) {
                LFS_SYS_FD_POOL.return_id(self.fd - 2).unwrap();
                LFS_SYS_OPEN_FDS.remove(&self.fd);
            }
            lfs::lfs_file_close(disk, self.file);
            lfs::lfs_sys_file_free(self.file);
        };
    }
}

impl FileHandle {
    /// Opens a file. Returns None if the file is already open (maybe by another thread)
    pub fn open(path: &str, flags: Option<&str>, mode: Option<i32>) -> Option<Self> {
        // todo: check for exclusive access ??
        let disk = disk();
        let existed = exists_sync_no_follow(path);
        let file = unsafe { lfs::lfs_sys_file_new() };
        let fd = (unsafe { LFS_SYS_FD_POOL.request_id() }).unwrap() + 2; // todo: randomize this
        let mut handle = Self {
            fd,
            file,
            path: path.to_string(),
        };
        let c_path = CString::new(path).unwrap();
        let flags = fs_flag_node_to_lfs(flags);
        let res = unsafe { lfs::lfs_file_open(disk, handle.file, c_path.as_ptr(), flags) };
        if res == lfs::lfs_error_LFS_ERR_OK {
            if !existed {
                unsafe { lfs::lfs_sys_attr_reset(c_path.as_ptr()) };
                let q = AttrQueryHandle::new(path);
                unsafe {
                    (*q.0).ino = LFS_SYS_INO_POOL.request_id()? as i32;
                    lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
                };
                handle.chmod(mode.unwrap_or(DEFAULT_PERM_FILE));
                Touch::birthtime(path, None);
            } else {
                Touch::atime(path, None);
            }
            Some(handle)
        } else {
            None
        }
    }
    /// Appends data to the file.
    pub fn append(&mut self, data: &[u8], length: Option<usize>) -> Option<()> {
        let disk = disk();
        self.sync();
        let length = length.unwrap_or(data.len()) as u32;
        let res = unsafe {
            let res = lfs::lfs_file_seek(
                disk,
                self.file,
                0,
                lfs::lfs_whence_flags_LFS_SEEK_END as i32,
            );
            assert_eq!(res, lfs::lfs_error_LFS_ERR_OK);
            lfs::lfs_file_write(disk, self.file, data.as_ptr() as *const c_void, length)
        };
        if res == lfs::lfs_error_LFS_ERR_OK {
            Touch::mtime(self.path.as_str(), None);
            Some(())
        } else {
            None
        }
    }
    /// Returns information about the file.
    pub fn stat(&self) -> Option<NodeStats> {
        stat_sync(self.path.as_str())
    }
    /// Changes the file's mode (permissions)
    pub fn chmod(&mut self, mode: i32) -> Option<()> {
        chmod_sync(self.path.as_str(), mode)
    }
    /// Changes the file's owner and group
    pub fn chown(&mut self, uid: i32, gid: i32) -> Option<()> {
        chown_sync(self.path.as_str(), uid, gid)
    }
    /// Changes the file's access and modification times
    pub fn utimes(&mut self, atime: f64, mtime: f64) -> Option<()> {
        utimes_sync(self.path.as_str(), atime, mtime)
    }
    /// Truncates the file to the specified length
    pub fn truncate(&mut self, len: u32) -> Option<()> {
        let disk = disk();
        self.sync();
        let res = unsafe { lfs::lfs_file_truncate(disk, self.file, len) };
        if res == lfs::lfs_error_LFS_ERR_OK {
            Touch::mtime(self.path.as_str(), None);
            Some(())
        } else {
            None
        }
    }
    /// Synchronizes the file's contents to disk
    pub fn sync(&mut self) -> Option<()> {
        let disk = disk();
        let res = unsafe { lfs::lfs_file_sync(disk, self.file) };
        if res == lfs::lfs_error_LFS_ERR_OK {
            Some(())
        } else {
            None
        }
    }
    /// Changes the file's offset
    pub fn lseek(&mut self, offset: i32, whence: i32) -> Option<i32> {
        let disk = disk();
        self.sync();
        let res = unsafe { lfs::lfs_file_seek(disk, self.file, offset, whence) };
        if res == lfs::lfs_error_LFS_ERR_OK {
            Some(res)
        } else {
            None
        }
    }
    /// Synchronizes the file's contents to disk
    pub fn datasync(&mut self) -> Option<()> {
        self.sync()
    }
    /// Reads data from the file
    pub fn read(
        &self,
        buf: &mut [u8],
        offset: Option<usize>,
        length: Option<usize>,
        position: Option<i32>,
    ) -> Option<usize> {
        let disk = disk();
        let seek = if let Some(position) = position {
            if position == -1 {
                false
            } else {
                true
            }
        } else {
            false
        };
        let offset = offset.unwrap_or(0);
        let length = length.unwrap_or(buf.len() - offset) as u32;
        let whence = lfs::lfs_whence_flags_LFS_SEEK_SET as i32;
        let buffer = unsafe { buf.as_mut_ptr().add(offset) as *mut c_void };
        if seek {
            unsafe {
                let curr = lfs::lfs_file_tell(disk, self.file);
                if curr < 0 {
                    return None;
                }
                let position = position.unwrap();
                lfs::lfs_file_seek(disk, self.file, position, whence);
                let res = lfs::lfs_file_read(disk, self.file, buffer, length);
                if res >= 0 {
                    lfs::lfs_file_seek(disk, self.file, curr, whence);
                    Touch::atime(self.path.as_str(), None);
                    Some(res as usize)
                } else {
                    None
                }
            }
        } else {
            unsafe {
                let res = lfs::lfs_file_read(disk, self.file, buffer, length);
                if res >= 0 {
                    Touch::atime(self.path.as_str(), None);
                    Some(res as usize)
                } else {
                    None
                }
            }
        }
    }
    /// Writes data to the file
    pub fn write(
        &mut self,
        buf: &[u8],
        offset: Option<usize>,
        length: Option<usize>,
        position: Option<i32>,
    ) -> Option<usize> {
        let disk = disk();
        self.sync();
        let seek = if let Some(position) = position {
            if position == -1 {
                false
            } else {
                true
            }
        } else {
            false
        };
        let offset = offset.unwrap_or(0);
        let length = length.unwrap_or(buf.len() - offset) as u32;
        let whence = lfs::lfs_whence_flags_LFS_SEEK_SET as i32;
        let buffer = unsafe { buf.as_ptr().add(offset) as *const c_void };
        if seek {
            unsafe {
                let curr = lfs::lfs_file_tell(disk, self.file);
                if curr < 0 {
                    return None;
                }
                let position = position.unwrap();
                lfs::lfs_file_seek(disk, self.file, position, whence);
                let res = lfs::lfs_file_write(disk, self.file, buffer, length);
                if res >= 0 {
                    lfs::lfs_file_seek(disk, self.file, curr, whence);
                    Touch::mtime(self.path.as_str(), None);
                    Some(res as usize)
                } else {
                    None
                }
            }
        } else {
            unsafe {
                let res = lfs::lfs_file_write(disk, self.file, buffer, length);
                if res >= 0 {
                    Touch::mtime(self.path.as_str(), None);
                    Some(res as usize)
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Debug)]
struct DirHandle {
    dir: *mut lfs::lfs_dir,
    info: *mut lfs::lfs_info,
    told: i32,
    pub path: String,
    pub fd: usize,
}

impl Drop for DirHandle {
    fn drop(&mut self) {
        let disk = disk();
        unsafe {
            if LFS_SYS_OPEN_FDS.contains_key(&self.fd) {
                LFS_SYS_FD_POOL.return_id(self.fd - 2).unwrap();
                LFS_SYS_OPEN_FDS.remove(&self.fd);
            }
            lfs::lfs_dir_close(disk, self.dir);
            lfs::lfs_sys_dir_free(self.dir);
            lfs::lfs_sys_info_free(self.info);
        }
    }
}

impl DirHandle {
    pub fn open(path: &str) -> Option<Self> {
        // todo: check for exclusive access ??
        let dir = unsafe { lfs::lfs_sys_dir_new() };
        let handle = Self {
            fd: (unsafe { LFS_SYS_FD_POOL.request_id() }).unwrap() + 2,
            dir,
            path: path.to_string(),
            info: unsafe { lfs::lfs_sys_info_new() },
            told: 0,
        };
        let disk = disk();
        let c_path = CString::new(handle.path.as_str()).unwrap();
        let res = unsafe { lfs::lfs_dir_open(disk, handle.dir, c_path.as_ptr()) };
        if res != lfs::lfs_error_LFS_ERR_OK {
            return None;
        }
        Touch::atime(path, None);
        Some(handle)
    }
    pub fn read(&mut self) -> Option<Dirent> {
        let disk = disk();
        loop {
            let res = unsafe { lfs::lfs_dir_read(disk, self.dir, self.info) };
            if res < 0 {
                return None;
            }
            if res == 0 {
                return None;
            }
            Touch::atime(self.path.as_str(), None);
            let tell = unsafe { lfs::lfs_dir_tell(disk, self.dir) };
            if tell <= self.told {
                return None;
            } else {
                self.told = tell;
            }
            let name = unsafe { (*self.info).name.as_ptr() };
            let name = unsafe { CStr::from_ptr(name) }.to_str().unwrap();
            if name == "." || name == ".." {
                continue;
            }
            let path = format!("{}/{}", self.path, name);
            return Some(Dirent::new(path.as_str()));
        }
    }
    /// Returns information about the directory
    pub fn stat(&self) -> Option<NodeStats> {
        stat_sync(self.path.as_str())
    }
    /// Changes the directory's mode (permissions)
    pub fn chmod(&mut self, mode: i32) -> Option<()> {
        chmod_sync(self.path.as_str(), mode)
    }
    /// Changes the directory's owner and group
    pub fn chown(&mut self, uid: i32, gid: i32) -> Option<()> {
        chown_sync(self.path.as_str(), uid, gid)
    }
    /// Changes the directory's access and modification times
    pub fn utimes(&mut self, atime: f64, mtime: f64) -> Option<()> {
        utimes_sync(self.path.as_str(), atime, mtime)
    }
}

/// ------------------------------------------------- f**(fd: **) api functions

pub fn lseek_sync(fd: usize, offset: i32, whence: i32) -> Option<i32> {
    match whence as u32 {
        lfs::lfs_whence_flags_LFS_SEEK_SET
        | lfs::lfs_whence_flags_LFS_SEEK_CUR
        | lfs::lfs_whence_flags_LFS_SEEK_END => match lookup_by_fd(fd) {
            None => None,
            Some(_) => {
                let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
                handle.as_mut().left()?.lseek(offset, whence)
            }
        },
        _ => return None,
    }
}

pub fn read_sync(
    fd: usize,
    buf: &mut [u8],
    offset: Option<usize>,
    length: Option<usize>,
    position: Option<i32>,
) -> Option<usize> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            handle.as_mut().left()?.read(buf, offset, length, position)
        }
    }
}

pub fn write_sync(
    fd: usize,
    buf: &[u8],
    offset: Option<usize>,
    length: Option<usize>,
    position: Option<i32>,
) -> Option<usize> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            handle.as_mut().left()?.write(buf, offset, length, position)
        }
    }
}

pub fn fstat(fd: usize) -> Option<NodeStats> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            match handle {
                Either::Left(file) => file.stat(),
                Either::Right(dir) => dir.stat(),
            }
        }
    }
}

pub fn fsync(fd: usize) -> Option<()> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            handle.as_mut().left()?.sync()
        }
    }
}

pub fn fdatasync(fd: usize) -> Option<()> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            handle.as_mut().left()?.datasync()
        }
    }
}

pub fn fchown(fd: usize, uid: i32, gid: i32) -> Option<()> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            match handle {
                Either::Left(file) => file.chown(uid, gid),
                Either::Right(dir) => dir.chown(uid, gid),
            }
        }
    }
}

pub fn fchmod(fd: usize, mode: i32) -> Option<()> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            match handle {
                Either::Left(file) => file.chmod(mode),
                Either::Right(dir) => dir.chmod(mode),
            }
        }
    }
}

pub fn ftruncate(fd: usize, len: usize) -> Option<()> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            handle.as_mut().left()?.truncate(len as u32);
            handle.as_mut().left()?.sync() // ??
        }
    }
}

pub fn futimes(fd: usize, atime: f64, mtime: f64) -> Option<()> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            match handle {
                Either::Left(file) => file.utimes(atime, mtime),
                Either::Right(dir) => dir.utimes(atime, mtime),
            }
        }
    }
}

pub fn freaddir_sync(fd: usize) -> Option<Dirent> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            let handle = unsafe { LFS_SYS_OPEN_FDS.get_mut(&fd) }?;
            handle.as_mut().right()?.read()
        }
    }
}

/// -------------------------------------------------- **Sync(**) api functions

pub fn open_file_sync(path: &str, flags: Option<&str>, mode: Option<i32>) -> Option<usize> {
    let handle = FileHandle::open(path, flags, mode)?;
    let fd = handle.fd;
    unsafe {
        LFS_SYS_OPEN_FDS.insert(fd, Left(handle));
    }
    Some(fd)
}

pub fn open_dir_sync(path: &str) -> Option<usize> {
    let handle = DirHandle::open(path)?;
    let fd = handle.fd;
    unsafe {
        LFS_SYS_OPEN_FDS.insert(fd, Right(handle));
    }
    Some(fd)
}

pub fn open_sync(path: &str, flags: Option<&str>, mode: Option<i32>) -> Option<usize> {
    if is_directory(path) {
        open_dir_sync(path)
    } else {
        open_file_sync(path, flags, mode)
    }
}

pub fn close_sync(fd: usize) -> Option<()> {
    match lookup_by_fd(fd) {
        None => None,
        Some(_) => {
            unsafe { LFS_SYS_OPEN_FDS.remove(&fd)? };
            return Some(());
        }
    }
}

fn exists_sync_no_follow(path: &str) -> bool {
    if path == "/" {
        return true;
    }
    if path == "" {
        return false;
    }
    let disk = disk();
    let info = InfoHandle::new();
    let path = CString::new(path).unwrap();
    let res = unsafe { lfs::lfs_stat(disk, path.as_ptr(), info.0) };
    res == lfs::lfs_error_LFS_ERR_OK
}

pub fn exists_sync(path: &str) -> bool {
    let path = follow_link(path, None);
    exists_sync_no_follow(&path)
}

pub fn link_sync(old_path: &str, new_path: &str) -> Result<JsValue, JsValue> {
    if !exists_sync(&old_path) {
        let err: JsValue = JsError::new("ENOENT: old_path does not exist").into();
        Reflect::set(&err, &"code".into(), &"ENOENT".into()).unwrap();
        Reflect::set(&err, &"path".into(), &old_path.into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"link".into()).unwrap();
        return Err(err);
    }
    if exists_sync(&new_path) {
        let err: JsValue = JsError::new("EEXIST: new_path already exists").into();
        Reflect::set(&err, &"code".into(), &"EEXIST".into()).unwrap();
        Reflect::set(&err, &"path".into(), &new_path.into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"link".into()).unwrap();
        return Err(err);
    }
    write_file_sync_no_follow(&new_path, old_path.as_bytes(), None, None);
    let n_path = CString::new(new_path).unwrap();
    let o_path = CString::new(old_path).unwrap();
    let n_attr = AttrQueryHandle::new(new_path);
    let o_attr = AttrQueryHandle::new(old_path);
    unsafe {
        // hard links have sync attributes and should not be visible to the user
        // as a "hard link". from user's perspective, hard links are just normal
        // files and directories. ".link" is sabfs internal attribute.
        (*n_attr.0).link = true;
        (*n_attr.0).nlink = 1;
        (*n_attr.0).symlink = (*o_attr.0).symlink;
        (*n_attr.0).birthtime = (*o_attr.0).birthtime;
        (*n_attr.0).atime = (*o_attr.0).atime;
        (*n_attr.0).mtime = (*o_attr.0).mtime;
        (*n_attr.0).ctime = (*o_attr.0).ctime;
        (*n_attr.0).uid = (*o_attr.0).uid;
        (*n_attr.0).gid = (*o_attr.0).gid;
        (*n_attr.0).mode = (*o_attr.0).mode;
        lfs::lfs_sys_attr_patch(n_path.as_ptr(), n_attr.0);
        (*o_attr.0).nlink += 1; // todo: handle deletes
        lfs::lfs_sys_attr_patch(o_path.as_ptr(), o_attr.0);
        if !LFS_SYS_HARD_LINKS.contains_key(old_path) {
            LFS_SYS_HARD_LINKS.insert(old_path.to_string(), vec![new_path.to_string()]);
        } else {
            let links = LFS_SYS_HARD_LINKS.get_mut(old_path).unwrap();
            links.push(new_path.to_string());
        }
    }
    Ok(JsValue::undefined())
}

pub fn symlink_sync(old_path: &str, new_path: &str) -> Result<JsValue, JsValue> {
    if !exists_sync(&old_path) {
        let err: JsValue = JsError::new("ENOENT: old_path does not exist").into();
        Reflect::set(&err, &"code".into(), &"ENOENT".into()).unwrap();
        Reflect::set(&err, &"path".into(), &old_path.into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"link".into()).unwrap();
        return Err(err);
    }
    if exists_sync(&new_path) {
        let err: JsValue = JsError::new("EEXIST: new_path already exists").into();
        Reflect::set(&err, &"code".into(), &"EEXIST".into()).unwrap();
        Reflect::set(&err, &"path".into(), &new_path.into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"link".into()).unwrap();
        return Err(err);
    }
    write_file_sync_no_follow(&new_path, old_path.as_bytes(), None, None);
    let n_path = CString::new(new_path).unwrap();
    let o_path = CString::new(old_path).unwrap();
    let n_attr = AttrQueryHandle::new(new_path);
    let o_attr = AttrQueryHandle::new(old_path);
    unsafe {
        // symlinks should be visible to the user as a "symlink".
        (*n_attr.0).link = false;
        (*n_attr.0).nlink = 1;
        (*n_attr.0).symlink = true;
        Touch::birthtime(new_path, None);
        (*n_attr.0).uid = 0;
        (*n_attr.0).gid = 0;
        lfs::lfs_sys_attr_patch(n_path.as_ptr(), n_attr.0);
        let is_dir = is_directory(old_path);
        let perms = get_default_permissions(is_dir);
        chmod_sync(new_path, perms);
        (*o_attr.0).nlink += 1; // todo: handle deletes
        lfs::lfs_sys_attr_patch(o_path.as_ptr(), o_attr.0);
    }
    Ok(JsValue::undefined())
}

pub fn mkdir_sync(path: &str, recursive: bool, mode: i32) -> Result<JsValue, JsValue> {
    if exists_sync(path) {
        if is_file(path) {
            let err: JsValue = JsError::new("EEXIST: file already exists, mkdir").into();
            Reflect::set(&err, &"code".into(), &"EEXIST".into()).unwrap();
            Reflect::set(&err, &"syscall".into(), &"mkdir".into()).unwrap();
            Err(err)
        } else {
            Ok(JsValue::undefined())
        }
    } else if recursive {
        let paths = path_split(path);
        let mut created_any = false;
        for p in paths.iter() {
            match mkdir_sync(p, false, mode) {
                Ok(_) => created_any = true,
                Err(err) => {
                    let code = Reflect::get(&err, &"code".into())
                        .unwrap()
                        .as_string()
                        .unwrap();
                    if code != "EEXIST" {
                        return Err(err);
                    }
                    let err: JsValue = JsError::new("ENOTDIR: not a directory, mkdir").into();
                    Reflect::set(&err, &"code".into(), &"ENOTDIR".into()).unwrap();
                    Reflect::set(&err, &"syscall".into(), &"mkdir".into()).unwrap();
                    Reflect::set(&err, &"path".into(), &path.into()).unwrap();
                    return Err(err);
                }
            }
        }
        if created_any {
            let ret = JsValue::from(paths.last().unwrap());
            Ok(ret)
        } else {
            Ok(JsValue::undefined())
        }
    } else {
        let path = follow_link(path, None);
        let parent = path_dirname(&path);
        let parent_q = AttrQueryHandle::new(&parent);
        let parent_perm: u32 = sanitize_permissions(unsafe { (*parent_q.0).mode });
        if (parent_perm & S_IWUSR) == 0
            && (parent_perm & S_IWGRP) == 0
            && (parent_perm & S_IWOTH) == 0
        {
            let err: JsValue = JsError::new("EACCES: permission denied, mkdir").into();
            Reflect::set(&err, &"code".into(), &"EACCES".into()).unwrap();
            Reflect::set(&err, &"syscall".into(), &"mkdir".into()).unwrap();
            Reflect::set(&err, &"path".into(), &path.into()).unwrap();
            return Err(err);
        }
        let disk = disk();
        let c_path = CString::new(path.clone()).unwrap();
        let res = unsafe { lfs::lfs_mkdir(disk, c_path.as_ptr()) };
        assert_eq!(res, lfs::lfs_error_LFS_ERR_OK);
        unsafe {
            lfs::lfs_sys_attr_reset(c_path.as_ptr());
            chmod_sync(&path, mode);
        };
        let q = AttrQueryHandle::new(&path);
        unsafe {
            (*q.0).ino = LFS_SYS_INO_POOL.request_id().unwrap() as i32;
            lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
        }
        Touch::birthtime(&path, None);
        let ret = JsValue::from(path);
        Ok(ret)
    }
}

pub fn mkdtemp_sync(prefix: &str) -> String {
    let path = format!("{}{}", prefix, rand_string());
    mkdir_sync(&path, false, DEFAULT_PERM_DIR).unwrap();
    path
}

pub fn readdir_sync(path: &str) -> Vec<Dirent> {
    let mut res = vec![];
    let mut handle = DirHandle::open(path).unwrap();
    while let Some(dirent) = handle.read() {
        res.push(dirent);
    }
    res
}

fn write_file_sync_no_follow(
    path: &str,
    data: &[u8],
    flags: Option<&str>,
    mode: Option<i32>,
) -> Option<()> {
    let flags = Some(flags.unwrap_or("w"));
    let mode = Some(mode.unwrap_or(DEFAULT_PERM_FILE));
    let mut handle = FileHandle::open(path, flags, mode)?;
    handle.write(data, None, None, None);
    handle.sync() // ??
}

pub fn write_file_sync(
    path: &str,
    data: &[u8],
    flags: Option<&str>,
    mode: Option<i32>,
) -> Option<()> {
    let path = follow_link(&path, None);
    write_file_sync_no_follow(&path, data, flags, mode)
}

fn read_file_sync_no_follow(path: &str) -> Option<Vec<u8>> {
    let handle = FileHandle::open(path, None, None)?;
    let mut data = vec![];
    loop {
        let mut buf = vec![0; 1024];
        if let Some(len) = handle.read(&mut buf, None, None, None) {
            if len == 0 {
                return Some(data);
            }
            data.extend_from_slice(&buf[..len]);
        } else {
            return Some(data);
        }
    }
}

pub fn read_file_sync(path: &str) -> Option<Vec<u8>> {
    let path = follow_link(&path, None);
    read_file_sync_no_follow(&path)
}

fn append_file_sync_no_follow(
    path: &str,
    data: &[u8],
    flags: Option<&str>,
    mode: Option<i32>,
) -> Option<()> {
    let flags = Some(flags.unwrap_or("a"));
    let mut handle = FileHandle::open(path, flags, mode)?;
    handle.append(data, None);
    Some(())
}

pub fn append_file_sync(
    path: &str,
    data: &[u8],
    flags: Option<&str>,
    mode: Option<i32>,
) -> Option<()> {
    let path = follow_link(&path, None);
    append_file_sync_no_follow(&path, data, flags, mode)
}

pub fn statfs_sync(path: &str, dump: Option<bool>) -> StatFs {
    let stat = StatHandle::new();
    let dump = dump.unwrap_or(false);
    StatFs {
        bsize: unsafe { (*stat.0).bsize as usize },
        blocks: unsafe { (*stat.0).blocks as usize },
        bfree: unsafe { (*stat.0).bfree as usize },
        bavail: unsafe { (*stat.0).bavail as usize },
        files: unsafe { (*stat.0).files as usize },
        ffree: unsafe { (*stat.0).ffree as usize },
        dirs: unsafe { (*stat.0).dirs as usize },
        json: if dump {
            Some(json_path(path).to_string())
        } else {
            None
        },
    }
}

pub fn chmod_sync(path: &str, perm: i32) -> Option<()> {
    let perm = sanitize_permissions(perm);
    let c_path = CString::new(path).ok()?;
    let q = AttrQueryHandle::new(path);
    unsafe {
        (*q.0).mode = if is_directory(path) {
            S_IFDIR | perm
        } else {
            S_IFREG | perm
        } as i32;
        lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
    }
    Touch::ctime(path, None);
    Some(())
}

pub fn chown_sync(path: &str, uid: i32, gid: i32) -> Option<()> {
    let c_path = CString::new(path).ok()?;
    let q = AttrQueryHandle::new(path);
    unsafe {
        (*q.0).uid = uid;
        (*q.0).gid = gid;
        lfs::lfs_sys_attr_patch(c_path.as_ptr(), q.0);
    }
    Touch::ctime(path, None);
    Some(())
}

pub fn truncate_sync(path: &str, size: usize) -> Option<()> {
    let mut handle = FileHandle::open(path, None, None)?;
    handle.truncate(size as u32);
    Some(())
}

pub fn utimes_sync(path: &str, atime: f64, mtime: f64) -> Option<()> {
    let path = &follow_link(&path, None);
    let atime = atime * 1000.0;
    let mtime = mtime * 1000.0;
    if exists_sync(path) {
        Touch::atime(path, Some(atime));
        Touch::mtime(path, Some(mtime));
        Some(())
    } else {
        None
    }
}

pub fn unlink_sync(path: &str, force: Option<bool>) -> Result<JsValue, JsValue> {
    let force = force.unwrap_or(false);
    let path = &follow_link(path, None);
    if is_directory(path) {
        let err: JsValue = JsError::new("EISDIR: illegal operation on a directory, unlink").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EISDIR"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    if !force && is_open(path) {
        let err: JsValue = JsError::new("EBUSY: resource busy or locked, unlink").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EBUSY"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    let disk = disk();
    let c_path = CString::new(path.clone()).unwrap();
    unsafe {
        lfs::lfs_remove(disk, c_path.as_ptr());
        if LFS_SYS_HARD_LINKS.contains_key(path) {
            for link in LFS_SYS_HARD_LINKS.get(path).unwrap() {
                let c_link = CString::new(link.clone()).unwrap();
                lfs::lfs_remove(disk, c_link.as_ptr());
            }
            LFS_SYS_HARD_LINKS.remove(path);
        }
    }

    Ok(JsValue::undefined())
}

pub fn rmdir_sync(path: &str, force: Option<bool>) -> Result<JsValue, JsValue> {
    let force = force.unwrap_or(false);
    let path = &follow_link(path, None);
    if is_file(path) {
        let err: JsValue = JsError::new("ENOTDIR: not a directory, rmdir").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("ENOTDIR"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    if !force && is_open(path) {
        let err: JsValue = JsError::new("EBUSY: resource busy or locked, rmdir").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EBUSY"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    let disk = disk();
    let c_path = CString::new(path.clone()).unwrap();
    let res = unsafe { lfs::lfs_remove(disk, c_path.as_ptr()) };
    if res == lfs::lfs_error_LFS_ERR_NOTEMPTY {
        let err: JsValue = JsError::new("ENOTEMPTY: directory not empty, rmdir").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("ENOTEMPTY"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    unsafe {
        if LFS_SYS_HARD_LINKS.contains_key(path) {
            for link in LFS_SYS_HARD_LINKS.get(path).unwrap() {
                let c_link = CString::new(link.clone()).unwrap();
                lfs::lfs_remove(disk, c_link.as_ptr());
            }
            LFS_SYS_HARD_LINKS.remove(path);
        }
    }
    Ok(JsValue::undefined())
}

pub fn rm_sync(path: &str, recursive: bool, force: bool) -> Result<JsValue, JsValue> {
    // todo: permission checks
    // todo: force is not fully implemented, once complete, should bypass thread locking
    if is_file(path) {
        return unlink_sync(path, Some(force));
    }
    if !force && is_open(path) {
        let err: JsValue = JsError::new("EBUSY: resource busy or locked, unlink").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EBUSY"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    if !recursive {
        return rmdir_sync(path, Some(force));
    }
    for ent in readdir_sync(path) {
        rm_sync(&format!("{}/{}", path, ent.name), true, force)?;
    }
    rm_sync(path, false, force)?;
    Ok(JsValue::undefined())
}

pub fn rename_sync(old_path: &str, new_path: &str) -> Option<bool> {
    if is_open(old_path) || is_open(new_path) {
        None
    } else {
        let disk = disk();
        let c_old_path = CString::new(old_path).unwrap();
        let c_new_path = CString::new(new_path).unwrap();
        let res = unsafe { lfs::lfs_rename(disk, c_old_path.as_ptr(), c_new_path.as_ptr()) };
        Touch::ctime(new_path, None);
        Some(res == lfs::lfs_error_LFS_ERR_OK)
    }
}

pub fn copy_file_sync(src: &str, dst: &str, excl: bool) -> Result<JsValue, JsValue> {
    let src = &follow_link(src, None);
    let dst = &follow_link(dst, None);
    if is_open(src) {
        let err: JsValue = JsError::new("EBUSY: resource busy or locked, copyFileSync").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EBUSY"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(src))?;
        return Err(err);
    }
    if is_open(dst) {
        let err: JsValue = JsError::new("EBUSY: resource busy or locked, copyFileSync").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EBUSY"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(dst))?;
        return Err(err);
    }
    if excl && exists_sync(dst) {
        let err: JsValue = JsError::new("EEXIST: file already exists, copyFileSync").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EEXIST"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(dst))?;
        return Err(err);
    }
    let src = read_file_sync(src).unwrap();
    write_file_sync(dst, &src, None, None);
    Ok(JsValue::undefined())
}

pub fn access_sync(path: &str, mode: Option<i32>) -> Result<JsValue, JsValue> {
    let path = &follow_link(path, None);
    if !exists_sync(path) {
        let err: JsValue = JsError::new(&format!(
            "ENOENT: no such file or directory, access '{}'",
            path
        ))
        .into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("ENOENT"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        Reflect::set(
            &err,
            &JsValue::from_str("syscall"),
            &JsValue::from_str("access"),
        )?;
        return Err(err);
    }
    let R_OK = 4;
    let W_OK = 2;
    let X_OK = 1;
    let F_OK = 0;
    let mode = mode.unwrap_or(F_OK);
    let info = InfoHandle::new();
    let c_path = CString::new(path.clone()).unwrap();
    let mut res = unsafe { lfs::lfs_stat(disk(), c_path.as_ptr(), info.0) };
    if res != lfs::lfs_error_LFS_ERR_OK {
        let err: JsValue =
            JsError::new("EFAULT: bad address in system call argument, access").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EFAULT"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    res = 0;
    res |= F_OK;
    let q = AttrQueryHandle::new(path);
    let test = unsafe { (*q.0).mode };
    // mode is just regular posix file permissions
    // https://en.wikipedia.org/wiki/File_system_permissions#POSIX_permissions
    if test & 0o400 != 0 {
        res |= R_OK;
    }
    if test & 0o200 != 0 {
        res |= W_OK;
    }
    if test & 0o100 != 0 {
        res |= X_OK;
    }
    if res & mode == mode {
        Ok(JsValue::undefined())
    } else {
        let err: JsValue = JsError::new("EACCES: permission denied, access").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EACCES"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        Err(err)
    }
}

pub fn readlink_sync(path: &str) -> Result<JsValue, JsValue> {
    // https://stackoverflow.com/a/1189582/388751
    if path.starts_with("/proc/self/fd/") {
        let fd = path
            .split("/")
            .last()
            .unwrap()
            .parse::<usize>()
            .unwrap_or(0);
        return if let Some(handle) = lookup_by_fd(fd) {
            Ok(JsValue::from_str(&match handle {
                Either::Left(file) => file.path.clone(),
                Either::Right(dir) => dir.path.clone(),
            }))
        } else {
            let err: JsValue = JsError::new("EBADF: bad file descriptor, readlink").into();
            Reflect::set(
                &err,
                &JsValue::from_str("code"),
                &JsValue::from_str("EBADF"),
            )?;
            Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
            Err(err)
        };
    }
    if !is_symlink(path) {
        let err: JsValue = JsError::new("EINVAL: invalid argument, readlink").into();
        Reflect::set(
            &err,
            &JsValue::from_str("code"),
            &JsValue::from_str("EINVAL"),
        )?;
        Reflect::set(&err, &JsValue::from_str("path"), &JsValue::from_str(path))?;
        return Err(err);
    }
    let path = realpath_sync(path, None);
    let content = read_file_sync(&path).unwrap();
    let content = String::from_utf8(content).unwrap();
    Ok(JsValue::from_str(&content))
}

pub fn realpath_sync(path: &str, attempt: Option<usize>) -> String {
    let path = &follow_link(path, None);
    let attempt = attempt.unwrap_or(0);
    assert!(attempt < 100); // prevent infinite recursion by death.
    if is_symlink(path) {
        if let Some(followed) = read_file_sync_no_follow(path) {
            return realpath_sync(
                String::from_utf8(followed).unwrap().as_str(),
                Some(attempt + 1),
            );
        }
    }
    path.to_string()
}

pub fn stat_sync(path: &str) -> Option<NodeStats> {
    let path = &follow_link(path, None);
    let q = AttrQueryHandle::new(path);
    Touch::atime(path, None);
    Some(unsafe {
        NodeStats {
            dev: lfs::lfs_sys_get_device_address(),
            ino: (*q.0).ino as f64,
            mode: (*q.0).mode as u16,
            nlink: (*q.0).nlink,
            uid: (*q.0).uid,
            gid: (*q.0).gid,
            rdev: 0,
            size: (*q.0).size,
            blksize: lfs::lfs_sys_get_block_size() as usize,
            blocks: lfs::lfs_sys_get_block_size() as usize,
            atimeMs: (*q.0).atime,
            mtimeMs: (*q.0).mtime,
            ctimeMs: (*q.0).ctime,
            birthtimeMs: (*q.0).birthtime,
        }
    })
}

pub fn lchmod_sync(path: &str, mode: i32) -> Option<()> {
    let path = realpath_sync(path, None);
    chmod_sync(&path, mode)
}

pub fn lchown_sync(path: &str, uid: i32, gid: i32) -> Option<()> {
    let path = realpath_sync(path, None);
    chown_sync(&path, uid, gid)
}

pub fn lutimes_sync(path: &str, atime: f64, mtime: f64) -> Option<()> {
    let path = realpath_sync(path, None);
    utimes_sync(&path, atime, mtime)
}

pub fn lstat_sync(path: &str) -> Option<NodeStats> {
    let path = realpath_sync(path, None);
    stat_sync(&path)
}

// ---------------------------------------------------------- Utility Functions

fn path_basename(path: &str) -> String {
    path.split("/").last().unwrap_or("").to_string()
}

fn path_dirname(path: &str) -> String {
    if path == "/" {
        "/".to_string()
    } else {
        let mut parts = path.split("/").collect::<Vec<&str>>();
        parts.pop();
        let parts = parts.join("/");
        if parts == "" {
            "/".to_string()
        } else {
            parts
        }
    }
}

fn path_normalize(path: &str) -> String {
    let path = path.to_string();
    if path == "." || path == ".." {
        return "/".to_string();
    }
    let mut normalized_path = Vec::new();
    for step in path.split("/") {
        if step == "." || step == "" {
            continue;
        }
        if step == ".." {
            normalized_path.pop();
            continue;
        }
        normalized_path.push(step);
    }
    let mut normalized_path = normalized_path.join("/");
    if normalized_path == "" {
        normalized_path = String::from("/");
    }
    if path.starts_with("/") && !normalized_path.starts_with("/") {
        normalized_path = format!("/{}", normalized_path);
    }
    normalized_path
}

pub fn path_split(path: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let components: Vec<&str> = path.split('/').collect();
    let mut current_path = String::new();

    for component in components {
        if !component.is_empty() {
            current_path.push('/');
            current_path.push_str(component);
            paths.push(current_path.clone());
        }
    }

    paths
}

fn json_file(path: &str) -> Value {
    let content = read_file_sync(path).unwrap();
    let attributes = AttrQueryHandle::new(path);
    json!({
            "path": path,
            "ino": unsafe { (*attributes.0).ino },
            "mode": unsafe { (*attributes.0).mode },
            "uid": unsafe { (*attributes.0).uid },
            "gid": unsafe { (*attributes.0).gid },
            "birthtime": unsafe { (*attributes.0).birthtime },
            "atime": unsafe { (*attributes.0).atime },
            "mtime": unsafe { (*attributes.0).mtime },
            "ctime": unsafe { (*attributes.0).ctime },
            "link": unsafe { (*attributes.0).link },
            "nlink": unsafe { (*attributes.0).nlink },
            "symlink": unsafe { (*attributes.0).symlink },
            "size": unsafe { (*attributes.0).size },
            "content": content,
    })
}

fn json_directory(path: &str) -> Value {
    let mut handle = DirHandle::open(path).unwrap();
    let mut children = vec![];
    while let Some(entry) = handle.read() {
        if entry.file {
            children.push(json_file(entry.name.as_str()));
        } else {
            children.push(json_directory(entry.name.as_str()));
        }
    }
    let attributes = AttrQueryHandle::new(path);
    json!({
            "path": path,
            "ino": unsafe { (*attributes.0).ino },
            "mode": unsafe { (*attributes.0).mode },
            "uid": unsafe { (*attributes.0).uid },
            "gid": unsafe { (*attributes.0).gid },
            "birthtime": unsafe { (*attributes.0).birthtime },
            "atime": unsafe { (*attributes.0).atime },
            "mtime": unsafe { (*attributes.0).mtime },
            "ctime": unsafe { (*attributes.0).ctime },
            "link": unsafe { (*attributes.0).link },
            "nlink": unsafe { (*attributes.0).nlink },
            "symlink": unsafe { (*attributes.0).symlink },
            "size": unsafe { (*attributes.0).size },
            "content": children,
    })
}

fn json_path(path: &str) -> Value {
    if is_directory(path) {
        json_directory(path)
    } else {
        json_file(path)
    }
}

fn follow_link(path: &str, attempt: Option<usize>) -> String {
    let attempt = attempt.unwrap_or(0);
    let path = &path_normalize(path);
    assert!(attempt < 100); // prevent infinite recursion by death.
    if is_link(path) {
        if let Some(followed) = read_file_sync_no_follow(path) {
            return follow_link(
                String::from_utf8(followed).unwrap().as_str(),
                Some(attempt + 1),
            );
        }
    }
    path.to_string()
}

fn rand_string() -> String {
    let now = Touch::time(None);
    let mut bits: u64 = now.to_bits();
    bits >>= 48;
    bits &= 0b0011_1111_1111;
    let base64_chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let hash_str: String = (0..6)
        .map(|i| {
            base64_chars
                .chars()
                .nth((bits >> (6 * i)) as usize & 0x3F)
                .unwrap()
        })
        .collect();
    hash_str
}

fn sanitize_permissions(p: i32) -> u32 {
    (p & (DEFAULT_PERM_DIR | DEFAULT_PERM_FILE)) as u32
}
