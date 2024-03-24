import path from "path";
import * as wasabio from "../../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.mkdtemp tests", () => {
	const tmpdir = "/tmp";

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
	});

	it("should create basic tempdir", () => {
		const tmpFolder = fs.mkdtempSync(path.join(tmpdir, "foo."));

		assert.strictEqual(path.basename(tmpFolder).length, "foo.XXXXXX".length);
		assert(fs.existsSync(tmpFolder));
	});

	it("should support utf8 in tempdir", () => {
		const utf8 = fs.mkdtempSync(path.join(tmpdir, "\u0222abc."));
		assert.strictEqual(Buffer.byteLength(path.basename(utf8)), Buffer.byteLength("\u0222abc.XXXXXX"));
		assert(fs.existsSync(utf8));
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
