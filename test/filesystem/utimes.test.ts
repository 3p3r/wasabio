import path from "path";
import * as wasabio from "../../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.utimes tests", () => {
	const tmpdir = "/tmp";

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
	});

	it("should check for Y2K38 support", function () {
		const testFilePath = path.join(tmpdir, "y2k38-test");
		const testFileDate = new Date("2040-01-02");

		fs.writeFileSync(testFilePath, "");
		fs.utimesSync(testFilePath, testFileDate, testFileDate);

		const dateResult = fs.statSync(testFilePath);
		assert.isNotNull(dateResult);
		assert.strictEqual(dateResult?.mtime?.getTime?.(), testFileDate.getTime());
	});

	it("should test utimes precision", () => {
		const testPath = path.join(tmpdir, "test-utimes-precision");
		fs.writeFileSync(testPath, "");

		const Y2K38_mtime = 2 ** 31;
		fs.utimesSync(testPath, Y2K38_mtime, Y2K38_mtime);
		const Y2K38_stats = fs.statSync(testPath);
		assert.strictEqual(Y2K38_stats?.mtime?.getTime()! / 1000.0, Y2K38_mtime);

		const truncate_mtime = 1713037251360;
		fs.utimesSync(testPath, truncate_mtime / 1000, truncate_mtime / 1000);
		const truncate_stats = fs.statSync(testPath);
		assert.strictEqual(truncate_stats?.mtime?.getTime(), truncate_mtime);

		const overflow_mtime = 2159345162531;
		fs.utimesSync(testPath, overflow_mtime / 1000.0, overflow_mtime / 1000.0);
		const overflow_stats = fs.statSync(testPath);
		assert.strictEqual(overflow_stats?.mtime?.getTime(), overflow_mtime);
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
