import path from "path";
import * as wasabio from "../../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.stat and fs.lstat", () => {
	const tmpdir = "/tmp";
	const __filename = "test.txt";

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
		fs.writeFileSync(path.join(tmpdir, __filename), "test");
	});

	it("should return stats with mtime as a Date and check for blksize and blocks", () => {
		const stats = fs.statSync(".");

		assert.ok(stats?.mtime instanceof Date);
		assert.isNumber(stats?.blksize);
		assert.isNumber(stats?.blocks);
	});

	it("should return lstat with mtime as a Date", () => {
		const stats = fs.lstatSync(".");
		assert.ok(stats?.mtime instanceof Date);
	});
	it("should return stats with mtime as a Date and check for the internal binding layer", () => {
		const fd = fs.openSync(".", "r");
		assert.ok(fd);

		// const st = fs.fstatSync(-0);
		// assert.ok(st);

		const stats = fs.fstatSync(fd);

		assert.ok(stats?.mtime instanceof Date);
		fs.closeSync(fd);
	});

	it("should return fstatSync with mtime as a Date", () => {
		const fd = fs.openSync(".", "r", undefined);

		const stats = fs.fstatSync(fd);
		assert.ok(stats?.mtime instanceof Date);
		fs.closeSync(fd);
	});

	it("should return stats with correct types and values for properties", (done) => {
		const s: any = fs.statSync(path.join(tmpdir, __filename));

		assert.strictEqual(s?.isDirectory?.(), false);
		assert.strictEqual(s?.isFile?.(), true);
		assert.strictEqual(s?.isSocket?.(), false);
		assert.strictEqual(s?.isBlockDevice?.(), false);
		assert.strictEqual(s?.isCharacterDevice?.(), false);
		assert.strictEqual(s?.isFIFO?.(), false);
		assert.strictEqual(s?.isSymbolicLink?.(), false);

		const jsonString = JSON.stringify(s);
		const parsed = JSON.parse(jsonString);

		const props = [
			"dev",
			"mode",
			"nlink",
			"uid",
			"gid",
			"rdev",
			"blksize",
			"ino",
			"size",
			"blocks",
			"atime",
			"mtime",
			"ctime",
			"birthtime",
			"atimeMs",
			"mtimeMs",
			"ctimeMs",
			"birthtimeMs",
		];

		for (let index = 0; index < props.length; index++) {
			const k: string = props[index];
			assert.isOk(k in s, `${k} should be in Stats`);
			assert.notStrictEqual(s?.[k], undefined, `${k} should not be undefined`);
			assert.notStrictEqual(s?.[k], null, `${k} should not be null`);
			assert.notStrictEqual(parsed[k], undefined, `${k} should not be undefined`);
			assert.notStrictEqual(parsed[k], null, `${k} should not be null`);
		}

		[
			"dev",
			"mode",
			"nlink",
			"uid",
			"gid",
			"rdev",
			"blksize",
			"ino",
			"size",
			"blocks",
			"atimeMs",
			"mtimeMs",
			"ctimeMs",
			"birthtimeMs",
		].forEach((k) => {
			assert.isNumber(s?.[k], `${k} should be a number`);
			assert.strictEqual(typeof parsed[k], "number", `${k} should be a number`);
		});
		["atime", "mtime", "ctime", "birthtime"].forEach((k) => {
			assert.ok(s?.[k] instanceof Date, `${k} should be a Date`);
			assert.strictEqual(typeof parsed[k], "string", `${k} should be a string`);
		});
		done();
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
