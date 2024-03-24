import path from "path";
import * as wasabio from "../../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.write tests", () => {
	const tmpdir = "/tmp";
	const fn = path.join(tmpdir, "write.txt");

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
	});

	it("should be able to do basic write", () => {
		const expected = "sample changed."; // Must be a unique string.
		const fd = fs.openSync(fn, "w");
		fs.writeSync(fd, expected, 0, "latin1");
		fs.closeSync(fd);
		assert.strictEqual(fs.readFileSync(fn), expected);
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
