import * as wasabio from "../../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.readdir tests", () => {
	const tmpdir = "/tmp";

	const readdirDir = tmpdir;
	const files = ["empty", "files", "for", "just", "testing"];

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
		// Create the necessary files
		files.forEach(function (currentFile) {
			fs.closeSync(fs.openSync(`${readdirDir}/${currentFile}`, "w"));
		});
	});

	it("should list files synchronously", () => {
		// Check the readdir Sync version
		assert.deepStrictEqual(files, fs.readdirSync(readdirDir).sort());
	});

	it("should throw ENOTDIR on file", () => {
		// readdir() on file should throw ENOTDIR
		// https://github.com/joyent/node/issues/1869
		assert.throws(function () {
			fs.readdirSync(`${tmpdir}/testing`);
		}, /Error: ENOTDIR: not a directory/);
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
