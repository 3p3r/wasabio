import * as wasabio from "../dist";
import Worker from "web-worker";
import { expect } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const isBrowser = globalThis.WASABIO !== undefined;
const {
	initialize,
	existsSync,
	statfsSync,
	readFileSync,
	mkdirSync,
	accessSync,
	statSync,
	writeSync,
	writeFileSync,
	lstatSync,
	chmodSync,
	utimesSync,
	openSync,
	readdirSync,
	readSync,
	unlinkSync,
	closeSync,
	rmdirSync,
	EventEmitter,
	localStorage,
} = isBrowser ? globalThis.WASABIO : wasabio;

describe("sanity testing", () => {
	let mem: WebAssembly.Memory;
	before(async () => {
		mem = await initialize();
	});

	it("should be able to read/write with Uint8Arrays", () => {
		const content = "hello world";
		const buffer = new TextEncoder().encode(content);
		writeFileSync("/test.txt", buffer);
		const buffer2 = readFileSync("/test.txt", { encoding: "buffer" });
		const content2 = new TextDecoder().decode(buffer2 as Uint8Array);
		expect(content2).to.equal(content);
		const buffer3 = readFileSync("/test.txt", { encoding: "buffer" });
		const content3 = new TextDecoder().decode(buffer3 as Uint8Array);
		expect(content3).to.equal(content);
	});

	it("should have a sane accessSync and chmodSync implementations", () => {
		expect(() => accessSync("/test", 0)).to.throw();
		expect(accessSync("/", 0)).to.be.undefined;
		writeFileSync("/test.txt", "Hello, World!", { encoding: "utf8" });
		chmodSync("/test.txt", 0o777);
		expect(accessSync("/test.txt", 0)).to.be.undefined;
		unlinkSync("/test.txt");
	});

	it("should be able to create and remove directories", () => {
		expect(existsSync("/")).to.be.true;
		const content = readdirSync("/");
		expect(content).to.be.an("array");
		expect(content).to.be.lengthOf(0);
		expect(existsSync("/test")).to.be.false;
		mkdirSync("/test");
		expect(existsSync("/test")).to.be.true;
		const { json } = statfsSync("/", true);
		const parsed = JSON.parse(json!);
		expect(parsed).to.have.property("content");
		expect(parsed.content).to.be.an("array");
		expect(parsed.content[0].path).to.equal("test");
		const content2 = readdirSync("/");
		expect(content2).to.be.an("array");
		expect(content2).to.be.lengthOf(1);
		expect(content2[0]).to.equal("test");
		rmdirSync("/test");
		expect(existsSync("/test")).to.be.false;
		const content3 = readdirSync("/");
		expect(content3).to.be.an("array");
		expect(content3).to.be.lengthOf(0);
	});

	it("should be able to read and write with FDs", () => {
		const fd = openSync("/test.txt", "w");
		expect(fd).to.be.greaterThan(0);
		const content = "Hello, World!";
		writeSync(fd, new TextEncoder().encode(content));
		closeSync(fd);
		const fd2 = openSync("/test.txt", "r");
		expect(fd2).to.be.greaterThan(0);
		const buf = new Uint8Array(content.length);
		const bytes = readSync(fd2, buf);
		expect(bytes).to.equal(content.length);
		const read = new TextDecoder().decode(buf);
		expect(read).to.equal(content);
		closeSync(fd2);
		unlinkSync("/test.txt");
	});

	it("should be able read and write basic files", () => {
		const content = "Hello, World!";
		mkdirSync("/test");
		expect(existsSync("/test/hello.txt")).to.be.false;
		writeFileSync("/test/hello.txt", content, { encoding: "utf8" });
		expect(existsSync("/test/hello.txt")).to.be.true;
		const read = readFileSync("/test/hello.txt", { encoding: "utf8" });
		expect(read).to.equal(content);
		unlinkSync("/test/hello.txt");
	});

	it("should have sane statSync and lstatSync implementations", () => {
		const content = "Hello, World!";
		mkdirSync("/test");
		expect(existsSync("/test/hello.txt")).to.be.false;
		writeFileSync("/test/hello.txt", content, { encoding: "utf8" });
		expect(existsSync("/test/hello.txt")).to.be.true;
		const stat = statSync("/test/hello.txt");
		expect(stat).to.have.property("dev");
		expect(stat).to.have.property("ino");
		expect(stat).to.have.property("mode");
		expect(stat).to.have.property("nlink");
		expect(stat).to.have.property("uid");
		expect(stat).to.have.property("gid");
		expect(stat).to.have.property("rdev");
		expect(stat).to.have.property("size");
		expect(stat).to.have.property("blksize");
		expect(stat).to.have.property("blocks");
		expect(stat).to.have.property("atimeMs");
		expect(stat).to.have.property("mtimeMs");
		expect(stat).to.have.property("ctimeMs");
		expect(stat).to.have.property("birthtimeMs");
		utimesSync("/test/hello.txt", Date.now(), Date.now());
		const lstat = lstatSync("/test/hello.txt");
		expect(lstat).to.have.property("dev");
		expect(lstat).to.have.property("ino");
		expect(lstat).to.have.property("mode");
		expect(lstat).to.have.property("nlink");
		expect(lstat).to.have.property("uid");
		expect(lstat).to.have.property("gid");
		expect(lstat).to.have.property("rdev");
		expect(lstat).to.have.property("size");
		expect(lstat).to.have.property("blksize");
		expect(lstat).to.have.property("blocks");
		expect(lstat).to.have.property("atimeMs");
		expect(lstat).to.have.property("mtimeMs");
		expect(lstat).to.have.property("ctimeMs");
		expect(lstat).to.have.property("birthtimeMs");
		unlinkSync("/test/hello.txt");
	});

	it("should be able to send and receive events on the same thread", (done) => {
		const emitter = new EventEmitter("emitter");
		emitter.once("test", (data) => {
			expect(data).to.equal("hello");
			done();
		});
		emitter.emit("test", "hello");
	});

	it("should be able to catch filesystem events", (done) => {
		const emitter = new EventEmitter("fs");
		emitter.once("mkdirSync", (data) => {
			expect(data).to.equal("/test2");
			done();
		});
		mkdirSync("/test2");
	});

	function createWorker() {
		// note: this gets replaced for browser testing, do not modify.
		return new Worker("./test/test.worker.js", { type: "module" });
	}

	it("should be able to send and receive events between threads", function (done) {
		const worker = createWorker();
		const emitter = new EventEmitter("worker");
		emitter.once("test", (data) => {
			expect(data).to.equal("hello");
			worker.terminate();
			done();
		});
		worker.postMessage({ mem, test: "events" });
	});

	it("should be able to catch filesystem events from a worker", function (done) {
		const worker = createWorker();
		const emitter = new EventEmitter("fs");
		emitter.once("mkdirSync", (data) => {
			expect(data).to.equal("/test3");
			worker.terminate();
			done();
		});
		worker.postMessage({ mem, test: "fs" });
	});

	it("should support a localStorage like api", () => {
		localStorage.setItem("hello", "world");
		expect(localStorage.getItem("hello")).to.equal("world");
		localStorage.removeItem("hello");
		expect(localStorage.getItem("hello")).to.be.null;
		localStorage.setItem("hello", "world");
		localStorage.setItem("foo", "bar");
		localStorage.clear();
		expect(localStorage.getItem("hello")).to.be.null;
		expect(localStorage.getItem("foo")).to.be.null;
		localStorage.setItem("hello", "world");
		localStorage.setItem("foo", "bar");
		expect([localStorage.key(0), localStorage.key(1)]).to.have.members(["hello", "foo"]);
		localStorage.removeItem("hello");
		localStorage.removeItem("foo");
	});
});
