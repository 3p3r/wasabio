import * as wasabio from "../../dist";
import { assert } from "chai";
import { join } from "path";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("fs.exists tests", () => {
	const tmpdir = "/tmp";
	const f = join(tmpdir, "read_write_file");

	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(tmpdir);
		fs.writeFileSync(f, "");
	});

	it("should export access constants", () => {
		assert.typeOf(fs.F_OK, "number");
		assert.typeOf(fs.R_OK, "number");
		assert.typeOf(fs.W_OK, "number");
		assert.typeOf(fs.X_OK, "number");
	});

	it("return truthy for valid accesses", () => {
		assert(fs.existsSync(f));
		assert(!fs.existsSync(`${f}-NO`));
	});

	it("should never throw", () => {
		// @ts-expect-error Testing no arguments
		assert(!fs.existsSync());
		// @ts-expect-error Testing invalid types
		assert(!fs.existsSync({}));
		assert(!fs.existsSync(new URL("https://foo")));
	});

	after(() => {
		fs.rmSync(tmpdir, { recursive: true, force: true });
	});
});
