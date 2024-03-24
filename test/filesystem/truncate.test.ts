import path from "path";
import * as wasabio from "../../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.truncate tests", () => {
	const tmpdir = "/tmp";
	const filename = path.join(tmpdir, "truncate-file.txt");
	const data = Buffer.alloc(1024 * 16, "x");
	let stat: any;

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
	});

	it("should check truncateSync functionality", function () {
		fs.writeFileSync(filename, data);
		stat = fs.statSync(filename);
		assert.strictEqual(stat.size, 1024 * 16);

		fs.truncateSync(filename, 1024);
		stat = fs.statSync(filename);
		assert.strictEqual(stat.size, 1024);

		fs.truncateSync(filename);
		stat = fs.statSync(filename);
		assert.strictEqual(stat.size, 0);
	});

	it("should check ftruncateSync functionality", function () {
		fs.writeFileSync(filename, data);
		const fd = fs.openSync(filename, "r+");

		assert.isNumber(fd);
		stat = fs.statSync(filename);
		assert.strictEqual(stat.size, 1024 * 16);

		fs.ftruncateSync(fd, 1024);
		stat = fs.statSync(filename);
		assert.strictEqual(stat.size, 1024);

		fs.ftruncateSync(fd);
		stat = fs.statSync(filename);
		assert.strictEqual(stat.size, 0);

		fs.closeSync(fd);
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
