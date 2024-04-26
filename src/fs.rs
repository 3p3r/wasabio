#![allow(non_snake_case)]

mod crt;
mod lfs;

use crate::bus::EventEmitter;
use defr::defr;
use js_sys::Reflect;
use nameof::name_of;
use once_cell::sync::Lazy;
use serde_json::json;
use wasm_bindgen::prelude::*;

static mut EMITTER: Lazy<EventEmitter> = Lazy::new(|| EventEmitter::new("fs"));

pub unsafe fn sab_fs_diag() {
    let stat = statfsSync("/".to_string(), Some(false));
    web_sys::console::log_1(&format!("[WASABIO:FS] statfsSync: {}", stat.json()).into());
    lfs::lfs_diag();
}

pub unsafe fn sab_fs_reboot() {
    EMITTER = Lazy::new(|| EventEmitter::new("fs"));
    lfs::lfs_reset();
}

pub unsafe fn sab_fs_locked() -> bool {
    lfs::lfs_locked()
}

struct ChangeType {}

impl ChangeType {
    const CHANGE: &'static str = "change";
    const RENAME: &'static str = "rename";
    const WATCH_: &'static str = "watch_";
}

fn parse_filesystem_mode(mode: String) -> i32 {
    if mode.starts_with("0o") {
        let mode = mode.replace("0o", "");
        i32::from_str_radix(mode.trim(), 8).unwrap()
    } else if mode.starts_with("0x") {
        let mode = mode.replace("0x", "");
        i32::from_str_radix(mode.trim(), 16).unwrap()
    } else if mode.starts_with("0b") {
        let mode = mode.replace("0b", "");
        i32::from_str_radix(mode.trim(), 2).unwrap()
    } else {
        i32::from_str_radix(mode.trim(), 8).unwrap()
    }
}

fn path_from_fd(fd: usize) -> String {
    lfs::readlink_sync(format!("/proc/self/fd/{}", fd).as_str())
        .unwrap()
        .as_string()
        .unwrap()
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "string | Uint8Array")]
    pub type UnionStringUint8Array;
    #[wasm_bindgen(typescript_type = "string | number")]
    pub type UnionStringNumber;
    #[wasm_bindgen(typescript_type = "object | undefined")]
    pub type UnionObjectUndefined;
}

// flip this to true to enable debug logging
const TRACE_FILESYSTEM_CALLS: bool = false;

macro_rules! debug_log {
	($name:expr, $($arg:expr),*) => {
		if TRACE_FILESYSTEM_CALLS {
			let ev = json!([ $($arg),* ]);
			let log_time = js_sys::Date::new_0().get_time();
			web_sys::console::log_1(&format!("[fs:{}] {}({})", log_time, $name, ev.to_string()).into());
		}
	};
}

macro_rules! broadcast_defer {
    ($name:expr, $($arg:expr),*) => {
		debug_log!($name, $($arg),*);
        let ev = json!([ $($arg),* ]);
        defr!(unsafe {
            EMITTER
                .emit($name.to_string(), ev.to_string())
                .unwrap_or(());
        });
    };
}

macro_rules! broadcast_watch {
    ($path:expr) => {
        let prevStat = lfs::stat_sync($path.as_str());
        defr!(unsafe {
            let currStat = lfs::stat_sync($path.as_str());
            let ev = json!([$path.to_string(), prevStat, currStat]);
            EMITTER
                .emit(ChangeType::WATCH_.to_string(), ev.to_string())
                .unwrap_or(());
        });
    };
}

#[wasm_bindgen]
pub fn linkSync(existing: String, path: String) -> Result<JsValue, JsValue> {
    broadcast_watch!(path);
    broadcast_watch!(existing);
    broadcast_defer!(ChangeType::RENAME, path);
    broadcast_defer!(ChangeType::CHANGE, existing);
    broadcast_defer!(name_of!(linkSync), existing, path);
    lfs::link_sync(existing.as_str(), path.as_str())
}

#[wasm_bindgen]
pub fn symlinkSync(target: String, path: String) -> Result<JsValue, JsValue> {
    broadcast_watch!(path);
    broadcast_watch!(target);
    broadcast_defer!(ChangeType::RENAME, path);
    broadcast_defer!(ChangeType::CHANGE, target);
    broadcast_defer!(name_of!(symlinkSync), target, path);
    lfs::symlink_sync(target.as_str(), path.as_str())
}

#[wasm_bindgen]
pub struct NodeStats {
    pub dev: f64,
    pub ino: f64,
    pub mode: f64,
    pub nlink: f64,
    pub uid: f64,
    pub gid: f64,
    pub rdev: f64,
    pub size: f64,
    pub blksize: f64,
    pub blocks: f64,
    pub atimeMs: f64,
    pub mtimeMs: f64,
    pub ctimeMs: f64,
    pub birthtimeMs: f64,
}

#[wasm_bindgen]
impl NodeStats {
    pub fn isFile(&self) -> bool {
        (self.mode as u32) & lfs::S_IFMT == lfs::S_IFREG
    }
    pub fn isDirectory(&self) -> bool {
        (self.mode as u32) & lfs::S_IFMT == lfs::S_IFDIR
    }
    pub fn isBlockDevice(&self) -> bool {
        false
    }
    pub fn isCharacterDevice(&self) -> bool {
        false
    }
    pub fn isFIFO(&self) -> bool {
        false
    }
    pub fn isSocket(&self) -> bool {
        false
    }
    pub fn isSymbolicLink(&self) -> bool {
        (self.mode as u32) & lfs::S_IFMT == lfs::S_IFLNK
    }
}

#[wasm_bindgen]
pub unsafe fn openSync(
    path: String,
    flags: Option<String>,
    mode: Option<UnionStringNumber>,
) -> Result<usize, JsValue> {
    let mode = if let Some(mode) = mode {
        Some(if mode.is_string() {
            parse_filesystem_mode(mode.as_string().unwrap())
        } else {
            mode.as_f64().unwrap() as i32
        })
    } else {
        None
    };
    let pathClone = path.clone();
    broadcast_watch!(pathClone);
    broadcast_defer!(name_of!(openSync), path, flags, mode);
    if let Some(fd) = lfs::open_sync(path.as_str(), flags.as_ref().map(|s| s.as_str()), mode) {
        return Ok(fd);
    } else {
        let err: JsValue = JsError::new("ENOENT: no such file or directory").into();
        Reflect::set(&err, &"path".into(), &path.into()).unwrap();
        Reflect::set(&err, &"code".into(), &"ENOENT".into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"open".into()).unwrap();
        return Err(err);
    }
}

#[wasm_bindgen]
pub unsafe fn opendirSync(path: String) -> Result<usize, JsValue> {
    broadcast_defer!(name_of!(opendirSync), path);
    openSync(path, None, None)
}

#[wasm_bindgen]
pub unsafe fn openfileSync(
    path: String,
    flags: Option<String>,
    mode: Option<UnionStringNumber>,
) -> Result<usize, JsValue> {
    broadcast_defer!(name_of!(openfileSync), path);
    openSync(path, flags, mode)
}

#[wasm_bindgen]
pub unsafe fn closeSync(fd: usize) -> Result<(), JsValue> {
    broadcast_defer!(name_of!(closeSync), fd);
    if lfs::close_sync(fd).is_some() {
        Ok(())
    } else {
        let err: JsValue = JsError::new("EBADF: bad file descriptor").into();
        Reflect::set(&err, &"code".into(), &"EBADF".into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"close".into()).unwrap();
        Err(err)
    }
}

#[wasm_bindgen]
pub unsafe fn lseekSync(fd: usize, offset: i32, whence: i32) -> i32 {
    broadcast_defer!(name_of!(lseekSync), fd, offset, whence);
    lfs::lseek_sync(fd, offset, whence).unwrap_or(-1)
}

#[wasm_bindgen]
pub unsafe fn readSync(
    fd: usize,
    buffer: &mut [u8],
    offset: Option<usize>,
    length: Option<usize>,
    position: Option<i32>,
) -> usize {
    broadcast_defer!(name_of!(readSync), fd, offset, length, position);
    lfs::read_sync(fd, buffer, offset, length, position).unwrap_or(0)
}

#[wasm_bindgen]
pub unsafe fn writeSync(
    fd: usize,
    buffer: &[u8],
    offset: Option<usize>,
    length: Option<usize>,
    position: Option<i32>,
) -> usize {
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(writeSync), fd, offset, length, position);
    broadcast_defer!(ChangeType::CHANGE, path_from_fd(fd));
    lfs::write_sync(fd, buffer, offset, length, position).unwrap_or(0)
}

#[wasm_bindgen]
pub unsafe fn fstatSync(fd: usize) -> NodeStats {
    broadcast_defer!(name_of!(fstatSync), fd);
    let stat = lfs::fstat(fd).unwrap();
    NodeStats {
        dev: stat.dev,
        ino: stat.ino,
        mode: stat.mode as f64,
        nlink: stat.nlink as f64,
        uid: stat.uid as f64,
        gid: stat.gid as f64,
        rdev: stat.rdev as f64,
        size: stat.size as f64,
        blksize: stat.blksize as f64,
        blocks: stat.blocks as f64,
        atimeMs: stat.atimeMs,
        mtimeMs: stat.mtimeMs,
        ctimeMs: stat.ctimeMs,
        birthtimeMs: stat.birthtimeMs,
    }
}

#[wasm_bindgen]
pub unsafe fn fchmodSync(fd: usize, mode: UnionStringNumber) {
    let mode = if mode.is_string() {
        parse_filesystem_mode(mode.as_string().unwrap())
    } else {
        mode.as_f64().unwrap() as i32
    };
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(fchmodSync), fd, mode);
    broadcast_defer!(ChangeType::CHANGE, path_from_fd(fd));
    lfs::fchmod(fd, mode).unwrap();
}

#[wasm_bindgen]
pub unsafe fn fchownSync(fd: usize, uid: usize, gid: usize) {
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(fchownSync), fd, uid, gid);
    broadcast_defer!(ChangeType::CHANGE, path_from_fd(fd));
    lfs::fchown(fd, uid as i32, gid as i32).unwrap();
}

#[wasm_bindgen]
pub unsafe fn ftruncateSync(fd: usize, len: Option<usize>) {
    let len = len.unwrap_or(0);
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(ftruncateSync), fd, len);
    broadcast_defer!(ChangeType::CHANGE, path_from_fd(fd));
    lfs::ftruncate(fd, len).unwrap();
}

#[wasm_bindgen]
pub unsafe fn futimesSync(fd: usize, atime: f64, mtime: f64) {
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(futimesSync), fd, atime, mtime);
    broadcast_defer!(ChangeType::CHANGE, path_from_fd(fd));
    lfs::futimes(fd, atime, mtime).unwrap();
}

#[wasm_bindgen]
pub unsafe fn fsyncSync(fd: usize) {
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(fsyncSync), fd);
    lfs::fsync(fd).unwrap();
}

#[wasm_bindgen]
pub unsafe fn fdatasyncSync(fd: usize) {
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(fdatasyncSync), fd);
    lfs::fdatasync(fd).unwrap()
}

#[wasm_bindgen]
pub unsafe fn existsSync(path: String) -> bool {
    broadcast_watch!(path);
    broadcast_defer!(name_of!(existsSync), path);
    lfs::exists_sync(path.as_str())
}

#[wasm_bindgen]
pub struct Dirent {
    name: String,
    path: String,
    file: bool,
    symlink: bool,
}

#[wasm_bindgen]
impl Dirent {
    pub fn isFile(&self) -> bool {
        self.file
    }
    pub fn isDirectory(&self) -> bool {
        !self.isFile()
    }
    pub fn isBlockDevice(&self) -> bool {
        false
    }
    pub fn isCharacterDevice(&self) -> bool {
        false
    }
    pub fn isFIFO(&self) -> bool {
        false
    }
    pub fn isSocket(&self) -> bool {
        false
    }
    pub fn isSymbolicLink(&self) -> bool {
        self.symlink
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn path(&self) -> String {
        self.path.clone()
    }
}

#[wasm_bindgen]
pub unsafe fn freaddirSync(fd: usize) -> Option<Dirent> {
    broadcast_watch!(path_from_fd(fd));
    broadcast_defer!(name_of!(freaddirSync), fd);
    let ent = lfs::freaddir_sync(fd)?;
    Some(
        (Dirent {
            file: ent.file,
            path: ent.path,
            name: ent.name,
            symlink: ent.symlink,
        })
        .into(),
    )
}

#[wasm_bindgen]
pub unsafe fn readdirSync(
    path: String,
    options: Option<UnionObjectUndefined>,
) -> Result<JsValue, JsValue> {
    let pathClone = path.clone();
    broadcast_watch!(pathClone);
    if !existsSync(path.clone()) {
        let err: JsValue = JsError::new("ENOENT: no such file or directory").into();
        Reflect::set(&err, &"path".into(), &path.into()).unwrap();
        Reflect::set(&err, &"code".into(), &"ENOENT".into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"readdir".into()).unwrap();
        return Err(err);
    }
    let stat = lfs::stat_sync(path.as_str()).unwrap();
    if (stat.mode as u32) & lfs::S_IFMT != lfs::S_IFDIR {
        let err: JsValue = JsError::new("Error: ENOTDIR: not a directory").into();
        Reflect::set(&err, &"code".into(), &"ENOTDIR".into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"readdir".into()).unwrap();
        return Err(err);
    }
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    let with_file_types = Reflect::get(&options, &"withFileTypes".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_bool()
        .unwrap_or(false);
    let arr = js_sys::Array::new();
    if with_file_types {
        broadcast_defer!(name_of!(readdirSync), path, with_file_types);
        for dirent in lfs::readdir_sync(path.as_str()) {
            arr.push(
                &(Dirent {
                    file: dirent.file,
                    path: dirent.path,
                    name: dirent.name,
                    symlink: dirent.symlink,
                })
                .into(),
            );
        }
    } else {
        broadcast_defer!(name_of!(readdirSync), path);
        for dirent in lfs::readdir_sync(path.as_str()) {
            arr.push(&dirent.name.into());
        }
    }
    Ok(arr.into())
}

#[wasm_bindgen]
pub unsafe fn mkdirSync(
    path: String,
    options: Option<UnionObjectUndefined>,
) -> Result<JsValue, JsValue> {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    let recursive = Reflect::get(&options, &"recursive".into())
        .unwrap_or_default()
        .as_bool()
        .unwrap_or(false);
    let mode = Reflect::get(&options, &"mode".into())
        .unwrap_or_default()
        .as_f64()
        .unwrap_or(lfs::DEFAULT_PERM_DIR as f64) as i32;
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::RENAME, path);
    broadcast_defer!(name_of!(mkdirSync), path, recursive, mode);
    lfs::mkdir_sync(path.as_str(), recursive, mode)
}

#[wasm_bindgen]
pub unsafe fn mkdtempSync(prefix: String) -> String {
    let ret = lfs::mkdtemp_sync(prefix.as_str());
    let retClone = ret.clone();
    // note: this is the only function that broadcasts the result
    broadcast_watch!(retClone);
    broadcast_defer!(ChangeType::RENAME, ret);
    broadcast_defer!(name_of!(mkdtempSync), prefix, ret);
    ret
}

#[wasm_bindgen]
pub unsafe fn writeFileSync(
    path: String,
    data: UnionStringUint8Array,
    options: Option<UnionObjectUndefined>,
) -> Result<(), JsError> {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    let encoding = Reflect::get(&options, &"encoding".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_string()
        .unwrap_or("utf8".to_string())
        .to_lowercase();
    let mode = Reflect::get(&options, &"mode".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_f64()
        .unwrap_or(0o666 as f64) as i32;
    let flag = Reflect::get(&options, &"flag".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_string()
        .unwrap_or("w".to_string())
        .to_lowercase();
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(writeFileSync), path);
    if encoding != "utf8" && encoding != "utf-8" && encoding != "buffer" {
        return Err(JsError::new("unsupported encoding"));
    }
    let data = if data.is_string() {
        data.as_string().unwrap().into_bytes()
    } else {
        js_sys::Uint8Array::new(&JsValue::from(&data)).to_vec()
    };
    lfs::write_file_sync(
        path.as_str(),
        data.as_slice(),
        Some(flag.as_str()),
        Some(mode),
    )
    .unwrap();
    Ok(())
}

#[wasm_bindgen]
pub unsafe fn readFileSync(
    path: String,
    options: Option<UnionObjectUndefined>,
) -> Result<UnionStringUint8Array, JsValue> {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    let encoding = Reflect::get(&options, &"encoding".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_string()
        .unwrap_or("utf8".to_string())
        .to_lowercase();
    if !existsSync(path.clone()) {
        let err: JsValue = JsError::new("ENOENT: no such file or directory").into();
        Reflect::set(&err, &"path".into(), &path.into()).unwrap();
        Reflect::set(&err, &"code".into(), &"ENOENT".into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"open".into()).unwrap();
        return Err(err);
    }
    if lfs::is_directory(&path) {
        let err: JsValue = JsError::new("EISDIR: illegal operation on a directory").into();
        Reflect::set(&err, &"code".into(), &"EISDIR".into()).unwrap();
        Reflect::set(&err, &"syscall".into(), &"read".into()).unwrap();
        return Err(err);
    }
    let pathClone = path.clone();
    broadcast_watch!(pathClone);
    broadcast_defer!(name_of!(readFileSync), path);
    let data = lfs::read_file_sync(path.as_str()).unwrap();
    let out = match encoding.as_str() {
        "utf8" | "utf-8" => {
            if String::from_utf8(data.clone()).is_ok() {
                JsValue::from(String::from_utf8(data).unwrap())
            } else {
                JsValue::from(js_sys::Uint8Array::from(data.as_slice()))
            }
        }
        "buffer" => JsValue::from(js_sys::Uint8Array::from(data.as_slice())),
        _ => return Err(JsError::new("unsupported encoding").into()),
    };
    Ok(UnionStringUint8Array::from(out))
}

#[wasm_bindgen]
pub unsafe fn appendFileSync(
    path: String,
    data: UnionStringUint8Array,
    options: Option<UnionObjectUndefined>,
) -> Result<(), JsError> {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    let encoding = Reflect::get(&options, &"encoding".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_string()
        .unwrap_or("utf8".to_string())
        .to_lowercase();
    let mode = Reflect::get(&options, &"mode".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_f64()
        .unwrap_or(0o666 as f64) as i32;
    let flag = Reflect::get(&options, &"flag".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_string()
        .unwrap_or("a".to_string())
        .to_lowercase();
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(appendFileSync), path);
    if encoding != "utf8" && encoding != "utf-8" && encoding != "buffer" {
        return Err(JsError::new("unsupported encoding"));
    }
    let data = if data.is_string() {
        data.as_string().unwrap().into_bytes()
    } else {
        js_sys::Uint8Array::new(&JsValue::from(&data)).to_vec()
    };
    lfs::append_file_sync(path.as_str(), &data, Some(flag.as_str()), Some(mode)).unwrap();
    Ok(())
}

#[wasm_bindgen]
pub struct StatFs {
    #[wasm_bindgen]
    pub bsize: usize,
    #[wasm_bindgen]
    pub blocks: usize,
    #[wasm_bindgen]
    pub bfree: usize,
    #[wasm_bindgen]
    pub bavail: usize,
    #[wasm_bindgen]
    pub files: usize,
    #[wasm_bindgen]
    pub ffree: usize,
    #[wasm_bindgen]
    pub dirs: usize,
    json: String,
}

#[wasm_bindgen]
impl StatFs {
    #[wasm_bindgen(getter)]
    pub fn json(&self) -> String {
        self.json.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_json(&mut self, json: String) {
        self.json = json;
    }
}

#[wasm_bindgen]
pub unsafe fn statfsSync(path: String, dump: Option<bool>) -> StatFs {
    broadcast_defer!(name_of!(statfsSync), path, dump);
    let stat = lfs::statfs_sync(path.as_str(), dump);
    StatFs {
        bsize: stat.bsize,
        blocks: stat.blocks,
        bfree: stat.bfree,
        bavail: stat.bavail,
        files: stat.files,
        ffree: stat.ffree,
        json: stat.json.unwrap_or("{}".to_string()),
        dirs: stat.dirs,
    }
}

#[wasm_bindgen]
pub unsafe fn chmodSync(path: String, mode: UnionStringNumber) {
    let mode = if mode.is_string() {
        parse_filesystem_mode(mode.as_string().unwrap())
    } else {
        mode.as_f64().unwrap() as i32
    };
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(chmodSync), path, mode);
    lfs::chmod_sync(path.as_str(), mode).unwrap();
}

#[wasm_bindgen]
pub unsafe fn chownSync(path: String, uid: usize, gid: usize) {
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(chownSync), path, uid, gid);
    lfs::chown_sync(path.as_str(), uid as i32, gid as i32).unwrap();
}

#[wasm_bindgen]
pub unsafe fn truncateSync(path: String, len: Option<usize>) {
    let len = len.unwrap_or(0);
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(truncateSync), path, len);
    lfs::truncate_sync(path.as_str(), len).unwrap();
}

#[wasm_bindgen]
pub unsafe fn utimesSync(path: String, atime: f64, mtime: f64) {
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(utimesSync), path, atime, mtime);
    lfs::utimes_sync(path.as_str(), atime, mtime).unwrap();
}

#[wasm_bindgen]
pub unsafe fn unlinkSync(path: String) -> Result<JsValue, JsValue> {
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::RENAME, path);
    broadcast_defer!(name_of!(unlinkSync), path);
    lfs::unlink_sync(path.as_str(), Some(true))
}

#[wasm_bindgen]
pub unsafe fn renameSync(old_path: String, new_path: String) {
    broadcast_watch!(old_path);
    broadcast_watch!(new_path);
    broadcast_defer!(ChangeType::RENAME, old_path);
    broadcast_defer!(ChangeType::RENAME, new_path);
    broadcast_defer!(name_of!(renameSync), old_path, new_path);
    lfs::rename_sync(old_path.as_str(), new_path.as_str()).unwrap();
}

#[wasm_bindgen]
pub unsafe fn copyFileSync(
    src: String,
    dest: String,
    options: Option<UnionObjectUndefined>,
) -> Result<JsValue, JsValue> {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    let COPYFILE_EXCL = 1;
    let mode = Reflect::get(&options, &"mode".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_f64()
        .unwrap_or(0.0) as i32;
    let excl = mode | COPYFILE_EXCL != 0;
    broadcast_watch!(src);
    broadcast_watch!(dest);
    broadcast_defer!(ChangeType::CHANGE, src);
    broadcast_defer!(ChangeType::RENAME, dest);
    broadcast_defer!(name_of!(copyFileSync), src, dest);
    lfs::copy_file_sync(src.as_str(), dest.as_str(), excl)
}

#[wasm_bindgen]
pub unsafe fn rmdirSync(path: String) {
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::RENAME, path);
    broadcast_defer!(name_of!(rmdirSync), path);
    lfs::rmdir_sync(path.as_str(), Some(false)).unwrap();
}

#[wasm_bindgen]
pub unsafe fn rmSync(path: String, options: Option<UnionObjectUndefined>) {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    let recursive = Reflect::get(&options, &"recursive".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_bool()
        .unwrap_or(false);
    let force = Reflect::get(&options, &"force".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_bool()
        .unwrap_or(false);
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::RENAME, path);
    broadcast_defer!(name_of!(rmSync), path, recursive, force);
    lfs::rm_sync(path.as_str(), recursive, force).unwrap();
}

#[wasm_bindgen]
pub unsafe fn accessSync(path: String, mode: Option<i32>) -> Result<JsValue, JsValue> {
    // broadcast_watch!(path); // deadlocks?
    broadcast_defer!(name_of!(accessSync), path, mode);
    lfs::access_sync(path.as_str(), mode)
}

#[wasm_bindgen]
pub unsafe fn realpathSync(path: String) -> String {
    broadcast_defer!(name_of!(realpathSync), path);
    lfs::realpath_sync(path.as_str(), None)
}

#[wasm_bindgen]
pub unsafe fn readlinkSync(path: String) -> Result<JsValue, JsValue> {
    broadcast_defer!(name_of!(readlinkSync), path);
    lfs::readlink_sync(path.as_str())
}

#[wasm_bindgen]
pub unsafe fn statSync(
    path: String,
    options: Option<UnionObjectUndefined>,
) -> Result<JsValue, JsValue> {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    broadcast_defer!(name_of!(statSync), path);
    let throw_if_no_entry = Reflect::get(&options, &"throwIfNoEntry".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_bool()
        .unwrap_or(true);
    if !existsSync(path.clone()) {
        if throw_if_no_entry {
            let err: JsValue = JsError::new("ENOENT: no such file or directory").into();
            Reflect::set(&err, &"path".into(), &path.into()).unwrap();
            Reflect::set(&err, &"code".into(), &"ENOENT".into()).unwrap();
            Reflect::set(&err, &"syscall".into(), &"stat".into()).unwrap();
            return Err(err);
        } else {
            return Ok(JsValue::undefined());
        }
    }
    let stat = lfs::stat_sync(path.as_str()).unwrap();
    let stat = (NodeStats {
        dev: stat.dev,
        ino: stat.ino,
        mode: stat.mode as f64,
        nlink: stat.nlink as f64,
        uid: stat.uid as f64,
        gid: stat.gid as f64,
        rdev: stat.rdev as f64,
        size: stat.size as f64,
        blksize: stat.blksize as f64,
        blocks: stat.blocks as f64,
        atimeMs: stat.atimeMs,
        mtimeMs: stat.mtimeMs,
        ctimeMs: stat.ctimeMs,
        birthtimeMs: stat.birthtimeMs,
    })
    .into();
    Ok(stat)
}

#[wasm_bindgen]
pub unsafe fn lchmodSync(path: String, mode: UnionStringNumber) {
    let mode = if mode.is_string() {
        parse_filesystem_mode(mode.as_string().unwrap())
    } else {
        mode.as_f64().unwrap() as i32
    };
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(lchmodSync), path, mode);
    lfs::lchmod_sync(path.as_str(), mode).unwrap();
}

#[wasm_bindgen]
pub unsafe fn lchownSync(path: String, uid: usize, gid: usize) {
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(lchownSync), path, uid, gid);
    lfs::lchown_sync(path.as_str(), uid as i32, gid as i32).unwrap();
}

#[wasm_bindgen]
pub unsafe fn lutimesSync(path: String, atime: f64, mtime: f64) {
    broadcast_watch!(path);
    broadcast_defer!(ChangeType::CHANGE, path);
    broadcast_defer!(name_of!(lutimesSync), path, atime, mtime);
    lfs::lutimes_sync(path.as_str(), atime, mtime).unwrap();
}

#[wasm_bindgen]
pub unsafe fn lstatSync(
    path: String,
    options: Option<UnionObjectUndefined>,
) -> Result<JsValue, JsValue> {
    let options = options.unwrap_or(UnionObjectUndefined::from(JsValue::undefined()));
    broadcast_defer!(name_of!(lstatSync), path);
    let throw_if_no_entry = Reflect::get(&options, &"throwIfNoEntry".into())
        .unwrap_or(JsValue::UNDEFINED)
        .as_bool()
        .unwrap_or(true);
    if !existsSync(path.clone()) {
        if throw_if_no_entry {
            let err: JsValue = JsError::new("ENOENT: no such file or directory").into();
            Reflect::set(&err, &"path".into(), &path.into()).unwrap();
            Reflect::set(&err, &"code".into(), &"ENOENT".into()).unwrap();
            Reflect::set(&err, &"syscall".into(), &"lstat".into()).unwrap();
            return Err(err);
        } else {
            return Ok(JsValue::undefined());
        }
    }
    let stat = lfs::lstat_sync(path.as_str()).unwrap();
    let stat = (NodeStats {
        dev: stat.dev,
        ino: stat.ino,
        mode: stat.mode as f64,
        nlink: stat.nlink as f64,
        uid: stat.uid as f64,
        gid: stat.gid as f64,
        rdev: stat.rdev as f64,
        size: stat.size as f64,
        blksize: stat.blksize as f64,
        blocks: stat.blocks as f64,
        atimeMs: stat.atimeMs,
        mtimeMs: stat.mtimeMs,
        ctimeMs: stat.ctimeMs,
        birthtimeMs: stat.birthtimeMs,
    })
    .into();
    Ok(stat)
}
