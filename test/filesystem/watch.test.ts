import * as wasabio from "../../dist";
import { assert } from "chai";
import path from "path";

declare global {
	var WASABIO: typeof wasabio;
}

const fs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;

var testDir = "/tmp";

var filenameOne = "watch.txt";
var filepathOne = path.join(testDir, filenameOne);

try {
	fs.unlinkSync(filepathOne);
} catch (e) {}

describe("fs.watch tests", () => {
	before(async () => {
		if (!fs.available()) await fs.initialize();
		fs.mkdirSync(testDir, { recursive: true });
	});

	it("should watch a file", function (done) {
		fs.writeFileSync(filepathOne, "hello");

		setTimeout(function () {
			fs.writeFileSync(filepathOne, "world");
		}, 20);

		var watcher = fs.watch(filepathOne);
		watcher.on("change", function (event, filename) {
			assert.equal("change", event);
			assert.isTrue(filename.toString().endsWith(filepathOne));
			watcher.close();
			done();
		});
	});

	after(() => {
    fs.unwatchFile('*');
		fs.rmSync(testDir, { recursive: true, force: true });
	});
});
