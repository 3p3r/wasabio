import * as wasabio from "../dist";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

describe("path utility tests", () => {
	before(async () => {
		if (!fs.available()) await fs.initialize();
	});

	it("should have sane join()", () => {
		assert.strictEqual(fs.join("/a", "b", "c"), "/a/b/c");
		assert.strictEqual(fs.join("/a", "b", "c/"), "/a/b/c");
	});

  it("should have sane dirname()", () => {
    assert.strictEqual(fs.dirname("/a/b/c"), "/a/b");
    assert.strictEqual(fs.dirname("/a/b/c/"), "/a/b");
  });

  it("should have sane basename()", () => {
    assert.strictEqual(fs.basename("/a/b/c"), "c");
    assert.strictEqual(fs.basename("/a/b/c/"), "c");
  });
});
