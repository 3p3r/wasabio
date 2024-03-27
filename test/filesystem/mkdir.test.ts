import * as wasabio from "../../dist";
import { assert } from "chai";
import { dirname, join } from "path";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.mkdir tests", () => {
	const tmpdir = "/tmp";

	let dirCount = 0;
	function nextDir() {
		return `test${++dirCount}`;
	}

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
	});

	it("should create directory with mode passed as an options object", () => {
		const pathname = join(tmpdir, nextDir());
		fs.mkdirSync(pathname, { mode: 0o777 });
		const exists = fs.existsSync(pathname);
		assert.strictEqual(exists, true);
	});

	it("should create directory from given path", () => {
		const pathname = join(tmpdir, nextDir());
		fs.mkdirSync(pathname);
		const exists = fs.existsSync(pathname);
		assert.strictEqual(exists, true);
	});

	it("should mkdirpSync when both top-level, and sub-folders do not exist", () => {
		const pathname = join(tmpdir, nextDir(), nextDir());
		fs.mkdirSync(pathname, { recursive: true });
		const exists = fs.existsSync(pathname);
		assert.strictEqual(exists, true);
		assert.strictEqual(fs.statSync(pathname)?.isDirectory?.(), true);
	});

	it("should mkdirpSync when folder already exists", () => {
		const pathname = join(tmpdir, nextDir(), nextDir());

		fs.mkdirSync(pathname, { recursive: true });
		// Should not cause an error.
		fs.mkdirSync(pathname, { recursive: true });

		const exists = fs.existsSync(pathname);
		assert.strictEqual(exists, true);
		assert.strictEqual(fs.statSync(pathname)?.isDirectory?.(), true);
	});

	it("should mkdirpSync ../", () => {
		const pathname = `${tmpdir}/${nextDir()}/../${nextDir()}/${nextDir()}`;
		assert.strictEqual(fs.existsSync(pathname), false);
		fs.mkdirSync(pathname, { recursive: true });
		assert.strictEqual(fs.existsSync(pathname), true);
		assert.strictEqual(fs.statSync(pathname)?.isDirectory?.(), true);
	});

	it("should throw when path is a file", () => {
		const pathname = join(tmpdir, nextDir(), nextDir());

		fs.mkdirSync(dirname(pathname));
		fs.writeFileSync(pathname, "", "utf8");

		assert.throws(() => {
			fs.mkdirSync(pathname);
		});

		try {
			fs.mkdirSync(pathname);
		} catch (err) {
			assert.strictEqual(err.code, "EEXIST");
			assert.strictEqual(err.syscall, "mkdir");
			assert.match(err.message, /EEXIST: .*mkdir/);
		}
	});

	it("should throw when part of the path is a file", () => {
		const filename = join(tmpdir, nextDir(), nextDir());
		const pathname = join(filename, nextDir(), nextDir());

		fs.mkdirSync(dirname(filename));
		fs.writeFileSync(filename, "", "utf8");

		assert.throws(() => {
			fs.mkdirSync(pathname, { recursive: true });
		});

		try {
			fs.mkdirSync(pathname, { recursive: true });
		} catch (err) {
			assert.strictEqual(err.path, pathname);
			assert.strictEqual(err.code, "ENOTDIR");
			assert.strictEqual(err.syscall, "mkdir");
			assert.match(err.message, /ENOTDIR: .*mkdir/);
		}
	});

	it("should return first folder created, when all folders are new", () => {
		const dir1 = nextDir();
		const dir2 = nextDir();
		const pathname = join(tmpdir, dir1, dir2);
		fs.mkdirSync(join(tmpdir, dir1), { recursive: true });
		const p = fs.mkdirSync(pathname, { recursive: true });
		assert.strictEqual(fs.existsSync(pathname), true);
		assert.strictEqual(fs.statSync(pathname)?.isDirectory?.(), true);
		assert.strictEqual(p, pathname);
	});

	it("should return first folder created, when last folder is new", () => {
		const dir1 = nextDir();
		const dir2 = nextDir();
		const pathname = join(tmpdir, dir1, dir2);
		fs.mkdirSync(join(tmpdir, dir1), { recursive: true });
		const p = fs.mkdirSync(pathname, { recursive: true });
		assert.strictEqual(fs.existsSync(pathname), true);
		assert.strictEqual(fs.statSync(pathname)?.isDirectory?.(), true);
		assert.strictEqual(p, pathname);
	});

	it("should return undefined, when no new folders are created", () => {
		const dir1 = nextDir();
		const dir2 = nextDir();
		const pathname = join(tmpdir, dir1, dir2);
		fs.mkdirSync(join(tmpdir, dir1, dir2), { recursive: true });
		const p = fs.mkdirSync(pathname, { recursive: true });
		assert.strictEqual(fs.existsSync(pathname), true);
		assert.strictEqual(fs.statSync(pathname)?.isDirectory?.(), true);
		assert.strictEqual(p, undefined);
	});

	it("should work when the lower bits of mode are > 0o777", () => {
		const mode = 0o644;
		const maskToIgnore = 0o10000;

		function test(mode: number, asString: boolean) {
			const suffix = asString ? "str" : "num";
			const input = asString ? (mode | maskToIgnore).toString(8) : mode | maskToIgnore;

			const dir = join(tmpdir, `mkdirSync-${suffix}`);
			fs.mkdirSync(dir, input);
			const m = fs.statSync(dir)?.mode;
			assert.isNumber(m);
			assert.strictEqual(m! & 0o777, mode);
		}

		test(mode, true);
		test(mode, false);
	});

	it("should throw appropriate errors when does not have permissions", () => {
		function makeDirectoryReadOnly(dir: string) {
			let accessErrorCode = "EACCES";
			fs.chmodSync(dir, "444");
			const mode = fs.statSync(dir)?.mode;
			assert.isNumber(mode);
			assert.strictEqual(mode! & 0o777, 0o444);
			return accessErrorCode;
		}

		const dir = join(tmpdir, "mkdirp_readonly");
		fs.mkdirSync(dir);
		const codeExpected = makeDirectoryReadOnly(dir);
		let err: any = null;
		try {
			fs.mkdirSync(join(dir, "/foo"), { recursive: true });
		} catch (_err) {
			err = _err;
		}
		assert.isNotNull(err);
		assert.strictEqual(err?.code, codeExpected);
		assert.strictEqual(err.path, join(dir, "/foo"));
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
