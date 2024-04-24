import init, {
	initSync,
	EventEmitter as NativeEmitter,
	sab_bus_receive,
	sab_bus_new,
	sab_bus_free,
	wasabio_locked,
	wasabio_reboot,
	wasabio_diag,
	NodeStats,
	Dirent,
	StatFs,
	sab_kv_set,
	sab_kv_get,
	sab_kv_del,
	sab_kv_key,
	sab_kv_clear,
	sab_kv_length,
} from "../pkg";
// @ts-ignore - handled by webpack, turns into base64
import WASM_BASE64 from "../pkg/wasabio_bg.wasm";
import type { EventEmitter as IEventEmitter } from "events"; // type only!
import type { Readable, Writable } from "stream";
import * as fs from "fs";
import JSZip from "jszip";
import atob from "atob-lite";
import toBuffer from "typedarray-to-buffer";
import { ok } from "assert";
import { Volume } from "memfs/lib/volume";
import { backOff } from "exponential-backoff";
import { callbackify } from "util";

const toUInt8 = (buf: any): Uint8Array =>
	buf instanceof ArrayBuffer
		? new Uint8Array(buf)
		: ArrayBuffer.isView(buf)
			? buf instanceof Uint8Array && buf.constructor.name === Uint8Array.name
				? buf
				: new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength)
			: new TextEncoder().encode(buf);

import { checksum, commit, built } from "../pkg/package.json";
const METADATA = { checksum, commit, built };

export interface Storage {
	setItem(key: string, value: string): void;
	getItem(key: string): string | null;
	removeItem(key: string): void;
	key(index: number): string | null;
	clear(): void;
	readonly length: number;
}

const INCORRECT_KEY_TYPE_ERROR_MESSAGE = "Incorrect key type. Expected a string.";
const INCORRECT_VAL_TYPE_ERROR_MESSAGE = "Incorrect value type. Expected a string.";
const INCORRECT_INDEX_TYPE_ERROR_MESSAGE = "Incorrect index type. Expected a number.";
const OBJECT_STORE_NAME = "memory";
const METADATA_NAME = "meta";

class LocalStorage implements Storage {
	setItem(key: string, value: string): void {
		ok(typeof key === "string", INCORRECT_KEY_TYPE_ERROR_MESSAGE);
		ok(typeof value === "string", INCORRECT_VAL_TYPE_ERROR_MESSAGE);
		sab_kv_set(key, value);
	}
	getItem(key: string): string | null {
		ok(typeof key === "string", INCORRECT_KEY_TYPE_ERROR_MESSAGE);
		return sab_kv_get(key);
	}
	removeItem(key: string): void {
		ok(typeof key === "string", INCORRECT_KEY_TYPE_ERROR_MESSAGE);
		sab_kv_del(key);
	}
	key(index: number): string | null {
		ok(typeof index === "number", INCORRECT_INDEX_TYPE_ERROR_MESSAGE);
		return sab_kv_key(index);
	}
	clear(): void {
		sab_kv_clear();
	}
	get length(): number {
		return sab_kv_length();
	}
}

export const localStorage: Storage = new LocalStorage();

function copyWebAssemblyMemoryToUint8Array(memory: WebAssembly.Memory): Uint8Array {
	const srcSize = memory.buffer.byteLength;
	const srcView = new Uint8Array(memory.buffer);
	const dstView = new Uint8Array(srcSize);
	dstView.set(srcView.subarray(0, srcSize));
	return dstView;
}

function copyUint8ArrayToWebAssemblyMemory(buffer: Uint8Array): WebAssembly.Memory {
	const memory = new WebAssembly.Memory({ initial: 18, maximum: 16384, shared: true });
	while (memory.buffer.byteLength < buffer.byteLength) memory.grow(1);
	const memoryView = new Uint8Array(memory.buffer);
	memoryView.set(buffer);
	return memory;
}

export interface InitializeOptions {
	readonly sync?: boolean;
	readonly reboot?: boolean;
}

let MEMORY: WebAssembly.Memory | undefined;
let THREAD_COUNTER_ADDRESS: number | undefined;

export function memory(): WebAssembly.Memory | undefined {
	return MEMORY;
}

export function initialize(
	mem?: WebAssembly.Memory | Uint8Array,
	opts?: InitializeOptions,
): Promise<WebAssembly.Memory> | WebAssembly.Memory {
	if (MEMORY) return MEMORY;
	if (ArrayBuffer.isView(mem)) {
		mem = copyUint8ArrayToWebAssemblyMemory(mem);
	}
	const post = (memory: WebAssembly.Memory, address: number) => {
		if (opts?.reboot) wasabio_reboot();
		THREAD_COUNTER_ADDRESS = address;
		MEMORY = memory;
		return memory;
	};
	if (opts?.sync) {
		const { memory, __wbindgen_thread_counter } = initSync(
			decodeWasmFromBase64String(WASM_BASE64 as any as string),
			mem,
		);
		return post(memory, __wbindgen_thread_counter());
	} else {
		return init(decodeWasmFromBase64String(WASM_BASE64 as any as string), mem).then(
			({ memory, __wbindgen_thread_counter }) => post(memory, __wbindgen_thread_counter()),
		);
	}
}

export function serialize(memory: WebAssembly.Memory): Uint8Array {
	const data = copyWebAssemblyMemoryToUint8Array(memory);
	if (THREAD_COUNTER_ADDRESS === undefined) return data;
	const view = new DataView(memory.buffer);
	view.setInt32(THREAD_COUNTER_ADDRESS, 0);
	return data;
}

export async function compress(buffer: Uint8Array): Promise<Uint8Array> {
	const zip = new JSZip();
	zip.file(OBJECT_STORE_NAME, buffer, { compression: "DEFLATE" });
	zip.file(METADATA_NAME, JSON.stringify(METADATA), { compression: "DEFLATE" });
	const zipBuffer = await zip.generateAsync({ type: "uint8array" });
	return zipBuffer;
}

export function deserialize(buffer: Uint8Array): WebAssembly.Memory {
	const memory = new WebAssembly.Memory({ initial: 18, maximum: 16384, shared: true });
	while (memory.buffer.byteLength < buffer.byteLength) memory.grow(1);
	const memoryView = new Uint8Array(memory.buffer);
	memoryView.set(buffer);
	return memory;
}

export async function decompress(buffer: Uint8Array): Promise<Uint8Array> {
	const zip = await JSZip.loadAsync(buffer);
	const zipBuffer = await zip.file(OBJECT_STORE_NAME)?.async("uint8array");
	const metadata = await zip.file(METADATA_NAME)?.async("string");
	ok(metadata, "no metadata");
	const { checksum, commit, built } = JSON.parse(metadata);
	ok(checksum, "no checksum");
	if (checksum === METADATA.checksum) {
		return zipBuffer;
	} else {
		throw new Error(`${checksum} != ${METADATA.checksum} (${commit}@${built})`);
	}
}

export function available(): boolean {
	return MEMORY !== undefined;
}

export function diagnostics(): void {
	if (MEMORY) wasabio_diag();
	else throw new Error("not initialized");
}

function decodeWasmFromBase64String(encoded: string) {
	const binaryString = atob(encoded);
	const bytes = new Uint8Array(binaryString.length);
	for (let i = 0; i < binaryString.length; i++) {
		bytes[i] = binaryString.charCodeAt(i);
	}
	return bytes.buffer;
}

interface BusDatum {
	readonly busId: number;
	readonly listener: (...args: any[]) => void;
	readonly eventName: string;
	readonly poller: NodeJS.Timeout;
}

export class EventEmitter implements IEventEmitter {
	private readonly _emitter: NativeEmitter;
	private readonly _busMap = new Set<BusDatum>();
	constructor(name?: string) {
		ok(MEMORY, "wasabio must be initialized before use.");
		name = name || `anonymous-${Date.now()}}`;
		this._emitter = new NativeEmitter(name);
	}
	private _dropBus(datum: BusDatum) {
		clearInterval(datum.poller);
		sab_bus_free(datum.busId);
		this._busMap.delete(datum);
	}
	private _recvBus(busId: number) {
		const ev = sab_bus_receive(busId);
		if (!ev) return null;
		try {
			return JSON.parse(ev);
		} catch (_) {
			ok(typeof ev === "string");
			return [ev];
		}
	}
	dispose() {
		this._emitter.free();
		for (const datum of this._busMap) {
			this._dropBus(datum);
		}
	}
	addListener(eventName: string, listener: (...args: any[]) => void): this {
		return this.on(eventName, listener);
	}
	on(eventName: string, listener: (...args: any[]) => void): this {
		const busId = sab_bus_new();
		ok(typeof busId === "number");
		const poller = setInterval(() => {
			const data = this._recvBus(busId);
			if (data) listener.apply(this, data);
		}, 0);
		this._busMap.add({ busId, listener, eventName, poller });
		this._emitter.on(eventName, busId);
		return this;
	}
	once(eventName: string, listener: (...args: any[]) => void): this {
		let busDatum: BusDatum;
		const busId = sab_bus_new();
		ok(typeof busId === "number");
		const poller = setInterval(() => {
			const data = this._recvBus(busId);
			if (data) {
				this.removeListener(eventName, listener);
				listener.apply(this, data);
			}
		}, 0);
		busDatum = { busId, listener, eventName, poller };
		this._busMap.add(busDatum);
		this._emitter.once(eventName, busId);
		return this;
	}
	removeListener(eventName: string, listener: (...args: any[]) => void): this {
		for (const datum of this._busMap) {
			if (datum.eventName === eventName && datum.listener === listener) {
				this._dropBus(datum);
				break;
			}
		}
		return this;
	}
	off(eventName: string, listener: (...args: any[]) => void): this {
		return this.removeListener(eventName, listener);
	}
	removeAllListeners(event?: string): this {
		if (event) {
			for (const datum of this._busMap) {
				if (datum.eventName === event) {
					this._dropBus(datum);
				}
			}
		} else {
			for (const datum of this._busMap) {
				this._dropBus(datum);
			}
		}
		return this;
	}
	setMaxListeners(n: number): this {
		this._emitter.setMaxListeners(n);
		return this;
	}
	getMaxListeners(): number {
		return this._emitter.getMaxListeners();
	}
	listeners(eventName: string): Function[] {
		const pipeIds = this._emitter.listeners(eventName);
		const listeners: Function[] = [];
		for (const pipeId of pipeIds) {
			for (const datum of this._busMap) {
				if (datum.busId === pipeId) {
					listeners.push(datum.listener);
					break;
				}
			}
		}
		return listeners;
	}
	rawListeners(eventName: string): Function[] {
		return this.listeners(eventName);
	}
	emit(eventName: string, ...args: any[]): boolean {
		const datum = JSON.stringify(args);
		this._emitter.emit(eventName, datum);
		return true;
	}
	listenerCount(eventName: string): number {
		return this.listeners(eventName).length;
	}
	prependListener(eventName: string, listener: (...args: any[]) => void): this {
		const busId = sab_bus_new();
		ok(typeof busId === "number");
		const poller = setInterval(() => {
			const data = this._recvBus(busId);
			if (data) listener.apply(this, data);
		}, 0);
		this._busMap.add({ busId, listener, eventName, poller });
		this._emitter.prependListener(eventName, busId);
		return this;
	}
	prependOnceListener(eventName: string, listener: (...args: any[]) => void): this {
		let busDatum: BusDatum;
		const busId = sab_bus_new();
		ok(typeof busId === "number");
		const poller = setInterval(() => {
			const data = this._recvBus(busId);
			if (data) {
				listener.apply(this, data);
				this._dropBus(busDatum);
			}
		}, 0);
		busDatum = { busId, listener, eventName, poller };
		this._busMap.add(busDatum);
		this._emitter.prependOnceListener(eventName, busId);
		return this;
	}
	eventNames(): string[] {
		return this._emitter.eventNames();
	}
}

const INVALID_PATH_ERROR_MESSAGE = "ERR_INVALID_PATH_TYPE";

export const F_OK = 0;
export const X_OK = 1;
export const W_OK = 2;
export const R_OK = 4;

export namespace constants {
	export const O_RDONLY = 0;
	export const O_WRONLY = 1;
	export const O_RDWR = 2;
	export const O_CREAT = 64;
	export const O_EXCL = 128;
	export const O_TRUNC = 512;
	export const O_APPEND = 1024;
	export const O_SYNC = 1052672;
}

export enum OpenMode {
	["r"] = constants.O_RDONLY,
	["r+"] = constants.O_RDWR,
	["rs"] = constants.O_RDONLY | constants.O_SYNC,
	["sr"] = OpenMode["rs"],
	["rs+"] = constants.O_RDWR | constants.O_SYNC,
	["sr+"] = OpenMode["rs+"],
	["w"] = constants.O_WRONLY | constants.O_CREAT | constants.O_TRUNC,
	["wx"] = constants.O_WRONLY | constants.O_CREAT | constants.O_TRUNC | constants.O_EXCL,
	["xw"] = OpenMode["wx"],
	["w+"] = constants.O_RDWR | constants.O_CREAT | constants.O_TRUNC,
	["wx+"] = constants.O_RDWR | constants.O_CREAT | constants.O_TRUNC | constants.O_EXCL,
	["xw+"] = OpenMode["wx+"],
	["a"] = constants.O_WRONLY | constants.O_APPEND | constants.O_CREAT,
	["ax"] = constants.O_WRONLY | constants.O_APPEND | constants.O_CREAT | constants.O_EXCL,
	["xa"] = OpenMode["ax"],
	["a+"] = constants.O_RDWR | constants.O_APPEND | constants.O_CREAT,
	["ax+"] = constants.O_RDWR | constants.O_APPEND | constants.O_CREAT | constants.O_EXCL,
	["xa+"] = OpenMode["ax+"],
}

type encoding = "utf8" | "utf-8" | "buffer";

function normalizePathLikeToString(path: fs.PathLike): string {
	let p: string | undefined;
	if (typeof path === "string") p = path;
	if (Buffer.isBuffer(path)) p = path.toString();
	p = p?.replace(/\\/g, "/");
	if (!p?.startsWith("/")) p = `/${p}`;
	if (p) return p;
	throw new Error(INVALID_PATH_ERROR_MESSAGE);
}

function timeLikeToSeconds(time: fs.TimeLike): number {
	return typeof time === "number" ? time : (typeof time === "string" ? Date.parse(time) : time.getTime()) / 1000;
}

function openModeLikeToString(flags?: fs.OpenMode): string {
	flags = flags ?? "r";
	flags =
		typeof flags === "number"
			? (Object.keys(OpenMode).find((key) => (OpenMode as any)[key] === flags) as string)
			: flags;
	return flags;
}

function moveNodeStatsToJsMemory(rustStatStruct?: NodeStats): fs.Stats | undefined {
	if (!rustStatStruct) return undefined;
	const _isDirectory = rustStatStruct.isDirectory();
	const _isFile = rustStatStruct.isFile();
	const _isBlockDevice = rustStatStruct.isBlockDevice();
	const _isCharacterDevice = rustStatStruct.isCharacterDevice();
	const _isSymbolicLink = rustStatStruct.isSymbolicLink();
	const _isFIFO = rustStatStruct.isFIFO();
	const _isSocket = rustStatStruct.isSocket();
	const clone = {
		atime: new Date(rustStatStruct.atimeMs),
		mtime: new Date(rustStatStruct.mtimeMs),
		ctime: new Date(rustStatStruct.ctimeMs),
		birthtime: new Date(rustStatStruct.birthtimeMs),
		atimeMs: rustStatStruct.atimeMs,
		mtimeMs: rustStatStruct.mtimeMs,
		ctimeMs: rustStatStruct.ctimeMs,
		birthtimeMs: rustStatStruct.birthtimeMs,
		blksize: rustStatStruct.blksize,
		blocks: rustStatStruct.blocks,
		dev: rustStatStruct.dev,
		gid: rustStatStruct.gid,
		ino: rustStatStruct.ino,
		mode: rustStatStruct.mode,
		nlink: rustStatStruct.nlink,
		rdev: rustStatStruct.rdev,
		size: rustStatStruct.size,
		uid: rustStatStruct.uid,
		isBlockDevice: () => _isBlockDevice,
		isCharacterDevice: () => _isCharacterDevice,
		isDirectory: () => _isDirectory,
		isFIFO: () => _isFIFO,
		isFile: () => _isFile,
		isSocket: () => _isSocket,
		isSymbolicLink: () => _isSymbolicLink,
	};
	rustStatStruct.free();
	return clone;
}

function moveDirentToJsMemory(rustDirentStruct?: Dirent): fs.Dirent | undefined {
	if (!rustDirentStruct) return undefined;
	const _isDirectory = rustDirentStruct.isDirectory();
	const _isFile = rustDirentStruct.isFile();
	const _isBlockDevice = rustDirentStruct.isBlockDevice();
	const _isCharacterDevice = rustDirentStruct.isCharacterDevice();
	const _isSymbolicLink = rustDirentStruct.isSymbolicLink();
	const _isFIFO = rustDirentStruct.isFIFO();
	const _isSocket = rustDirentStruct.isSocket();
	const clone = {
		name: rustDirentStruct.name,
		path: rustDirentStruct.path,
		isBlockDevice: () => _isBlockDevice,
		isCharacterDevice: () => _isCharacterDevice,
		isDirectory: () => _isDirectory,
		isFIFO: () => _isFIFO,
		isFile: () => _isFile,
		isSocket: () => _isSocket,
		isSymbolicLink: () => _isSymbolicLink,
	};
	rustDirentStruct.free();
	return clone;
}

function moveStatFsToJsMemory(rustStatFsStruct: StatFs): fs.StatsFs {
	const clone = {
		bfree: rustStatFsStruct.bfree,
		bavail: rustStatFsStruct.bavail,
		blocks: rustStatFsStruct.blocks,
		bsize: rustStatFsStruct.bsize,
		ffree: rustStatFsStruct.ffree,
		files: rustStatFsStruct.files,
		json: rustStatFsStruct.json,
	};
	rustStatFsStruct.free();
	return clone as unknown as fs.StatsFs;
}

// re-export all of the fs functions
import { linkSync } from "../pkg";
export { linkSync };
import { symlinkSync } from "../pkg";
export { symlinkSync };
import { openSync as _openSync } from "../pkg";
export function openSync(path: fs.PathLike, flags?: fs.OpenMode, mode?: fs.Mode): number {
	path = normalizePathLikeToString(path);
	return _openSync(path, openModeLikeToString(flags), mode);
}
import { opendirSync } from "../pkg";
export { opendirSync };
import { openfileSync } from "../pkg";
export { openfileSync };
import { closeSync } from "../pkg";
export { closeSync };
import { readSync as _readSync } from "../pkg";
export function readSync(fd: number, buffer: Uint8Array, offset?: number, length?: number, position?: number): number;
export function readSync(fd: number, buffer: Uint8Array, opts?: fs.ReadSyncOptions): number;
export function readSync(fd: number, buffer: Uint8Array, ...args: any[]): number {
	const isOpts = typeof args?.[0] === "object";
	let offset = isOpts ? args?.[0]?.offset : args?.[0];
	let length = isOpts ? args?.[0]?.length : args?.[1];
	let position = isOpts ? args?.[0]?.position : args?.[2];
	return _readSync(fd, buffer, offset, length, position);
}
import { writeSync as _writeSync } from "../pkg";
export function writeSync(
	fd: number,
	buffer: Uint8Array,
	offset?: number | null,
	length?: number | null,
	position?: number | null,
): number;
export function writeSync(
	fd: number,
	string: string,
	position?: number | null,
	encoding?: BufferEncoding | null,
): number;
export function writeSync(fd: number, data: string | Uint8Array, ...args: any[]): number {
	let offset = 0;
	let position = 0;
	let length: number;
	let buffer: Uint8Array;
	if (typeof data === "string") {
		buffer = new TextEncoder().encode(data);
		length = buffer.length;
		position = args?.[0] || 0;
	} else if (typeof args?.[0] === "object") {
		buffer = data;
		offset = args?.[0]?.offset || 0;
		length = args?.[0]?.length || buffer.length;
		position = args?.[0]?.position || 0;
	} else {
		buffer = data;
		offset = args?.[0] || 0;
		length = args?.[1] || buffer.length;
		position = args?.[2] || 0;
	}
	return _writeSync(fd, buffer, offset, length, position);
}
import { fstatSync as _fstatSync } from "../pkg";
export function fstatSync(fd: number): Partial<fs.Stats> | undefined {
	return moveNodeStatsToJsMemory(_fstatSync(fd));
}
import { fchmodSync } from "../pkg";
export { fchmodSync };
import { fchownSync } from "../pkg";
export { fchownSync };
import { ftruncateSync } from "../pkg";
export { ftruncateSync };
import { futimesSync as _futimesSync } from "../pkg";
export function futimesSync(fd: number, atime: fs.TimeLike, mtime: fs.TimeLike): void {
	_futimesSync(fd, timeLikeToSeconds(atime), timeLikeToSeconds(mtime));
}
import { fsyncSync } from "../pkg";
export { fsyncSync };
import { fdatasyncSync } from "../pkg";
export { fdatasyncSync };
import { existsSync as _existsSync } from "../pkg";
export function existsSync(path: fs.PathLike): boolean {
	try {
		path = normalizePathLikeToString(path);
		return _existsSync(path);
	} catch (e: any) {
		if (e?.message === INVALID_PATH_ERROR_MESSAGE) return false;
		throw e;
	}
}
import { freaddirSync as _freaddirSync } from "../pkg";
export function freaddirSync(fd: number): fs.Dirent | undefined {
	return moveDirentToJsMemory(_freaddirSync(fd));
}
import { readdirSync as _readdirSync } from "../pkg";
export function readdirSync(
	path: fs.PathLike,
	options?: { withFileTypes?: boolean; recursive?: boolean | undefined },
): fs.Dirent[] | string[] {
	path = normalizePathLikeToString(path);
	return options?.withFileTypes ? _readdirSync(path, options).map(moveDirentToJsMemory) : _readdirSync(path, options);
}
import { mkdirSync as _mkdirSync } from "../pkg";
export function mkdirSync(path: fs.PathLike, options?: fs.MakeDirectoryOptions | string | number): string | undefined {
	if (typeof options === "number") {
		options = { mode: options };
	}
	if (typeof options === "string") {
		options = { mode: parseInt(options, 8) };
	}
	path = normalizePathLikeToString(path);
	return _mkdirSync(path, options);
}
import { mkdtempSync } from "../pkg";
export { mkdtempSync };
import { writeFileSync as _writeFileSync } from "../pkg";
export function writeFileSync(
	pathOrFd: fs.PathLike | number,
	data: Buffer | Uint8Array | string,
	options?: object | encoding,
): void {
	if (typeof options === "string") {
		options = { encoding: options };
	}
	if (typeof pathOrFd === "number") {
		throw new Error("not implemented, use writeSync");
	} else {
		_writeFileSync(normalizePathLikeToString(pathOrFd), toUInt8(data), options);
	}
}
import { readFileSync as _readFileSync } from "../pkg";
export function readFileSync(pathOrFd: fs.PathLike | number, options?: object | encoding): Buffer | string {
	if (typeof options === "string") {
		options = { encoding: options };
	}
	if (typeof pathOrFd === "number") {
		throw new Error("not implemented, use readSync");
	} else {
		pathOrFd = normalizePathLikeToString(pathOrFd);
		const content = _readFileSync(pathOrFd, options);
		return typeof content === "string" ? content : toBuffer(content);
	}
}
import { appendFileSync as _appendFileSync } from "../pkg";
export function appendFileSync(
	pathOrFd: fs.PathLike | number,
	data: Buffer | Uint8Array | string,
	options?: object | encoding,
): void {
	if (typeof options === "string") {
		options = { encoding: options };
	}
	if (typeof pathOrFd === "number") {
		throw new Error("not implemented, use appendSync");
	} else {
		pathOrFd = normalizePathLikeToString(pathOrFd);
		_appendFileSync(pathOrFd, toUInt8(data), options);
	}
}
import { statfsSync } from "../pkg";
export { statfsSync };
import { chmodSync as _chmodSync } from "../pkg";
export function chmodSync(path: fs.PathLike, mode: fs.Mode): void {
	path = normalizePathLikeToString(path);
	_chmodSync(path, mode);
}
import { chownSync as _chownSync } from "../pkg";
export function chownSync(path: fs.PathLike, uid: number, gid: number): void {
	path = normalizePathLikeToString(path);
	_chownSync(path, uid, gid);
}
import { truncateSync as _truncateSync } from "../pkg";
export function truncateSync(pathOrFd: fs.PathLike | number, len?: number): void {
	if (typeof pathOrFd === "number") {
		throw new Error("not implemented, use truncateSync");
	} else {
		pathOrFd = normalizePathLikeToString(pathOrFd);
		_truncateSync(pathOrFd, len);
	}
}
import { utimesSync as _utimesSync } from "../pkg";
export function utimesSync(path: fs.PathLike, atime: fs.TimeLike, mtime: fs.TimeLike): void {
	path = normalizePathLikeToString(path);
	_utimesSync(path, timeLikeToSeconds(atime), timeLikeToSeconds(mtime));
}
import { unlinkSync as _unlinkSync } from "../pkg";
export function unlinkSync(path: fs.PathLike): void {
	path = normalizePathLikeToString(path);
	_unlinkSync(path);
}
import { renameSync as _renameSync } from "../pkg";
export function renameSync(oldPath: fs.PathLike, newPath: fs.PathLike): void {
	oldPath = normalizePathLikeToString(oldPath);
	newPath = normalizePathLikeToString(newPath);
	_renameSync(oldPath, newPath);
}
import { copyFileSync } from "../pkg";
export { copyFileSync };
import { rmdirSync } from "../pkg";
export { rmdirSync };
import { rmSync } from "../pkg";
export { rmSync };
import { accessSync as _accessSync } from "../pkg";
export function accessSync(path: fs.PathLike, mode?: number): void {
	path = normalizePathLikeToString(path);
	_accessSync(path, mode);
}
import { realpathSync } from "../pkg";
export { realpathSync };
import { readlinkSync } from "../pkg";
export { readlinkSync };
import { statSync as _statSync } from "../pkg";
export function statSync(path: fs.PathLike, options?: { throwIfNoEntry: boolean }): Partial<fs.Stats> | undefined {
	path = normalizePathLikeToString(path);
	return moveNodeStatsToJsMemory(_statSync(path, options));
}
import { lchmodSync as _lchmodSync } from "../pkg";
export function lchmodSync(path: fs.PathLike, mode: fs.Mode): void {
	path = normalizePathLikeToString(path);
	_lchmodSync(path, mode);
}
import { lchownSync as _lchownSync } from "../pkg";
export function lchownSync(path: fs.PathLike, uid: number, gid: number): void {
	path = normalizePathLikeToString(path);
	_lchownSync(path, uid, gid);
}
import { lutimesSync as _lutimesSync } from "../pkg";
export function lutimesSync(path: fs.PathLike, atime: fs.TimeLike, mtime: fs.TimeLike): void {
	path = normalizePathLikeToString(path);
	_lutimesSync(path, timeLikeToSeconds(atime), timeLikeToSeconds(mtime));
}
import { lstatSync as _lstatSync } from "../pkg";
export function lstatSync(path: fs.PathLike, options?: { bigint: boolean }): Partial<fs.Stats> | undefined {
	path = normalizePathLikeToString(path);
	return moveNodeStatsToJsMemory(_lstatSync(path, options));
}
import { lseekSync as _lseekSync } from "../pkg";
export function lseekSync(fd: number, offset: number, whence: number): number {
	return _lseekSync(fd, offset, whence);
}

const backOffOpts: any = {
	delayFirstAttempt: false,
	numOfAttempts: 100,
	maxDelay: 60000,
};

const delayForLock = async () => {
	await backOff(async () => {
		if (wasabio_locked()) throw new Error();
	}, backOffOpts);
};

const delayedBackOff = async <T>(fn: () => Promise<T>): Promise<T> => {
	await delayForLock().catch(() => {});
	return await backOff(fn, backOffOpts);
};

function promisify<T extends (...args: any[]) => any>(fn: T): (...args: Parameters<T>) => Promise<ReturnType<T>> {
	return (...args: Parameters<T>) => {
		return delayedBackOff(() => fn(...args));
	};
}

/**
 * note: if you are accessing the filesystem in a webworker, you can use either
 * sync or async versions of the functions. if you are accessing the filesystem
 * on the main thread or another UI context (webview extensions), you must use
 * the async versions of the functions. this is because the sync versions of
 * the functions will block the main thread and cause the UI to freeze.
 *
 * async version simply backs off exponentially until the operation succeeds.
 *
 * callback versions are just wrappers around the async versions.
 */

export namespace promises {
	export const link = promisify(linkSync);
	export const symlink = promisify(symlinkSync);
	export const open = promisify(openSync);
	export const opendir = promisify(opendirSync);
	export const openfile = promisify(openfileSync);
	export const close = promisify(closeSync);
	export const lseek = promisify(lseekSync);
	export const read = promisify(readSync);
	export const write = promisify(writeSync);
	export const fstat = promisify(fstatSync);
	export const fchmod = promisify(fchmodSync);
	export const fchown = promisify(fchownSync);
	export const ftruncate = promisify(ftruncateSync);
	export const futimes = promisify(futimesSync);
	export const fsync = promisify(fsyncSync);
	export const fdatasync = promisify(fdatasyncSync);
	export const exists = promisify(existsSync);
	export const freaddir = promisify(freaddirSync);
	export const readdir = promisify(readdirSync);
	export const mkdir = promisify(mkdirSync);
	export const mkdtemp = promisify(mkdtempSync);
	export const writeFile = promisify(writeFileSync);
	export const readFile = promisify(readFileSync);
	export const appendFile = promisify(appendFileSync);
	export const statfs = promisify(statfsSync);
	export const chmod = promisify(chmodSync);
	export const chown = promisify(chownSync);
	export const truncate = promisify(truncateSync);
	export const utimes = promisify(utimesSync);
	export const unlink = promisify(unlinkSync);
	export const rename = promisify(renameSync);
	export const copyFile = promisify(copyFileSync);
	export const rmdir = promisify(rmdirSync);
	export const rm = promisify(rmSync);
	export const access = promisify(accessSync);
	export const realpath = promisify(realpathSync);
	export const readlink = promisify(readlinkSync);
	export const stat = promisify(statSync);
	export const lchmod = promisify(lchmodSync);
	export const lchown = promisify(lchownSync);
	export const lutimes = promisify(lutimesSync);
	export const lstat = promisify(lstatSync);
}

export const link = callbackify(promises.link);
export const symlink = callbackify(promises.symlink);
export const open = callbackify(promises.open);
export const opendir = callbackify(promises.opendir);
export const openfile = callbackify(promises.openfile);
export const close = callbackify(promises.close);
export const lseek = callbackify(promises.lseek);
export const read = callbackify(promises.read);
export const write = callbackify(promises.write);
export const fstat = callbackify(promises.fstat);
export const fchmod = callbackify(promises.fchmod);
export const fchown = callbackify(promises.fchown);
export const ftruncate = callbackify(promises.ftruncate);
export const futimes = callbackify(promises.futimes);
export const fsync = callbackify(promises.fsync);
export const fdatasync = callbackify(promises.fdatasync);
export const exists = callbackify(promises.exists);
export const freaddir = callbackify(promises.freaddir);
export const readdir = callbackify(promises.readdir);
export const mkdir = callbackify(promises.mkdir);
export const mkdtemp = callbackify(promises.mkdtemp);
export const writeFile = callbackify(promises.writeFile);
export const readFile = callbackify(promises.readFile);
export const appendFile = callbackify(promises.appendFile);
export const statfs = callbackify(promises.statfs);
export const chmod = callbackify(promises.chmod);
export const chown = callbackify(promises.chown);
export const truncate = callbackify(promises.truncate);
export const utimes = callbackify(promises.utimes);
export const unlink = callbackify(promises.unlink);
export const rename = callbackify(promises.rename);
export const copyFile = callbackify(promises.copyFile);
export const rmdir = callbackify(promises.rmdir);
export const rm = callbackify(promises.rm);
export const access = callbackify(promises.access);
export const realpath = callbackify(promises.realpath);
export const readlink = callbackify(promises.readlink);
export const stat = callbackify(promises.stat);
export const lchmod = callbackify(promises.lchmod);
export const lchown = callbackify(promises.lchown);
export const lutimes = callbackify(promises.lutimes);
export const lstat = callbackify(promises.lstat);

const emptyVolume = new Volume();

export function createReadStream(path: fs.PathLike, options?: string | object): Readable {
	const _opts = typeof options === "string" ? { encoding: options } : options || {};
	return new emptyVolume.ReadStream.prototype.__proto__.constructor({ open, read, close }, path, _opts) as Readable;
}
export function createWriteStream(path: fs.PathLike, options?: string | object): Writable {
	const _opts = typeof options === "string" ? { encoding: options } : options || {};
	return new emptyVolume.WriteStream.prototype.__proto__.constructor({ open, write, close }, path, _opts) as Writable;
}

class Singleton<T> {
	private _instance: T | undefined;
	constructor(private readonly _factory: () => T) {}
	get(): T {
		if (!this._instance) {
			this._instance = this._factory();
		}
		return this._instance;
	}
}

const fileSystemSharedEmitter = new Singleton(() => new EventEmitter("fs"));
const exposedSurfacedEmitters = new Set<Watcher>();

class Watcher extends EventEmitter implements fs.FSWatcher, fs.StatWatcher {
	private _refs = 0;
	constructor(
		readonly path: string,
		readonly watcher: Function,
		private readonly cleanup?: Function,
	) {
		super();
		exposedSurfacedEmitters.add(this);
		this.on("change", () => {
			if (!exposedSurfacedEmitters.has(this)) {
				this.close();
			}
		});
	}
	ref() {
		this._refs++;
		return this;
	}
	unref() {
		this._refs--;
		if (this._refs <= 0) this.close();
		return this;
	}
	close(): void {
		exposedSurfacedEmitters.delete(this);
		this._refs = 0;
		this.cleanup?.();
		this.emit("close");
		this.removeAllListeners();
	}
}

// @ts-ignore
export const watch: typeof fs.watch = (filename, ...args): Watcher => {
	const opts = (args.find((arg) => typeof arg === "object") || {}) as fs.WatchOptions;
	const listener = (args.find((arg) => typeof arg === "function") || (() => {})) as Function;
	const _filename = normalizePathLikeToString(filename);
	const watcher = new Watcher(_filename, listener);
	watcher.on("change", listener as any);
	const _listener = (event: string, file: string) => {
		if (opts.recursive) {
			if (file.startsWith(_filename)) {
				watcher.emit("change", event, file);
			}
		} else {
			if (file === _filename) {
				watcher.emit("change", event, file);
			}
		}
	};
	const _c = _listener.bind(null, "change");
	const _r = _listener.bind(null, "rename");
	fileSystemSharedEmitter.get().on("change", _c);
	fileSystemSharedEmitter.get().on("rename", _r);
	return watcher.once("close", () => {
		fileSystemSharedEmitter.get().off("change", _c);
		fileSystemSharedEmitter.get().off("rename", _r);
	});
};

// @ts-ignore
export const watchFile: typeof fs.watchFile = (filename, ...args): Watcher => {
	// const opts = (args.find((arg) => typeof arg === "object") || {}) as fs.WatchFileOptions;
	const listener = (args.find((arg) => typeof arg === "function") || (() => {})) as Function;
	const watcher = new Watcher(normalizePathLikeToString(filename), listener);
	watcher.on("change", listener as any);
	const _listener = (curr?: Partial<fs.Stats>, prev?: Partial<fs.Stats>) => {
		const now = new Date();
		const nowStat = {
			atime: now,
			mtime: now,
			ctime: now,
			birthtime: now,
			atimeMs: now.getTime(),
			mtimeMs: now.getTime(),
			ctimeMs: now.getTime(),
			birthtimeMs: now.getTime(),
			blksize: 0,
			blocks: 0,
			dev: 0,
			gid: 0,
			ino: 0,
			mode: 0,
			nlink: 0,
			rdev: 0,
			size: 0,
			uid: 0,
			isBlockDevice: () => false,
			isCharacterDevice: () => false,
			isDirectory: () => false,
			isFIFO: () => false,
			isFile: () => false,
			isSocket: () => false,
			isSymbolicLink: () => false,
		};
		const _curr = { ...nowStat, ...curr };
		const _prev = { ...nowStat, ...prev };
		watcher.emit("change", _curr, _prev);
	};
	fileSystemSharedEmitter.get().on("watch_", _listener);
	return watcher.once("close", () => {
		fileSystemSharedEmitter.get().off("watch_", _listener);
	});
};

// @ts-ignore
export const unwatchFile: typeof fs.unwatchFile = (filename, ...args): void => {
	if (filename === "*") {
		fileSystemSharedEmitter.get().removeAllListeners();
		return exposedSurfacedEmitters.clear();
	}
	const _filename = normalizePathLikeToString(filename);
	const listener = (args.find((arg) => typeof arg === "function") || (() => {})) as Function;
	for (const emitter of exposedSurfacedEmitters) {
		if (listener ? emitter.watcher === listener : emitter.path === _filename) {
			emitter.close();
		}
	}
};
