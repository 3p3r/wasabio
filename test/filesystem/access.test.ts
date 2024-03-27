import * as wasabio from "../../dist";
import { assert } from "chai";
import { join } from "path";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.access tests", () => {
	const tmpdir = "/tmp";
	const doesNotExist = join(tmpdir, "__this_should_not_exist");
	const readOnlyFile = join(tmpdir, "read_only_file");
	const readWriteFile = join(tmpdir, "read_write_file");

	function createFileWithPerms(file: string, mode: number) {
		fs.writeFileSync(file, "");
		fs.chmodSync(file, mode);
	}

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
		createFileWithPerms(readOnlyFile, 0o444);
		createFileWithPerms(readWriteFile, 0o666);
	});

	it("should export access constants", () => {
		assert.typeOf(fs.F_OK, "number");
		assert.typeOf(fs.R_OK, "number");
		assert.typeOf(fs.W_OK, "number");
		assert.typeOf(fs.X_OK, "number");
	});

	it("should not throw with regular access", () => {
		const mode = fs.R_OK | fs.W_OK;
		fs.accessSync(readWriteFile, mode);
		fs.accessSync(readWriteFile, fs.R_OK);
	});

	it("should throw with no access", () => {
		assert.throws(() => fs.accessSync(doesNotExist));
		try {
			fs.accessSync(doesNotExist);
		} catch (err) {
			assert.strictEqual(err.code, "ENOENT");
			assert.strictEqual(err.path, doesNotExist);
			assert.strictEqual(err.message, `ENOENT: no such file or directory, access '${doesNotExist}'`);
			assert.strictEqual(err.constructor, Error);
			assert.strictEqual(err.syscall, "access");
		}
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
