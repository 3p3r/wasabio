import * as wasabio from "../../dist";
import { assert } from "chai";
import { join } from "path";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.link tests", () => {
	const tmpdir = "/tmp";
	const srcPath = join(tmpdir, "hardlink-target.txt");
	const dstPath = join(tmpdir, "link1.js");

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
		fs.writeFileSync(srcPath, "hello world");
	});

	it("should be able to create and read hard links", () => {
		fs.linkSync(srcPath, dstPath);
		const dstContent = fs.readFileSync(dstPath, "utf8");
		assert.strictEqual(dstContent, "hello world");
	});

	it("should be able to delete hard links", () => {
		assert.isTrue(fs.existsSync(dstPath));
		fs.unlinkSync(dstPath);
		assert.isFalse(fs.existsSync(dstPath));
		// deleting hard links causes the original file to be deleted
		assert.isFalse(fs.existsSync(srcPath));
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
