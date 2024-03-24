import path from "path";
import * as wasabio from "../../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.read tests", () => {
	let fd: number;
	const tmpdir = "/tmp";
	const filepath = path.join(tmpdir, "x.txt");
	const expected = Buffer.from("xyz\n");

	function test(bufferSync: Buffer | Uint8Array, expected: Buffer | Uint8Array) {
		const r = fs.readSync(fd, bufferSync, 0, expected.length, 0);
		assert.deepStrictEqual(bufferSync, expected);
		assert.strictEqual(r, expected.length);
	}

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
		fs.writeFileSync(filepath, expected);
		fd = fs.openSync(filepath, "r");
	});

	it("should be able to do basic read", () => {
		test(Buffer.allocUnsafe(expected.length), expected);
		test(new Uint8Array(expected.length), Uint8Array.from(expected));
	});

	after(() => {
		fs.closeSync(fd);
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
