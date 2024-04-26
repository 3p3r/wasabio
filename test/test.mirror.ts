// this test is isolated because "memfs" does not clean after itself up properly

import * as wasabio from "../dist";
import externalFS from "memfs";
import { assert } from "chai";

declare global {
	var WASABIO: typeof wasabio;
}

const internalFs = globalThis.WASABIO !== undefined ? globalThis.WASABIO : wasabio;
const { join } = internalFs;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const internalDir = "/a/b/c";
const externalDir = "/nested";

let cleanup: () => void;

async function before() {
	if (!internalFs.available()) await internalFs.initialize();
	externalFS.vol.reset();
	const mirrored = await internalFs.mirror({
		externalFS: externalFS.fs as any,
		externalDir,
		internalDir,
		fileFilter: (f) => f.endsWith(".txt"),
		logger: console.log,
	});
	cleanup = mirrored.close;
}

async function main() {
	console.log("mirror utility tests");

	await internalFs.promises.writeFile(join(internalDir, "/file.txt"), "hello world 1", "utf8");
	await sleep(500);
	assert.strictEqual(externalFS.fs.readFileSync(join(externalDir, "/file.txt"), "utf8"), "hello world 1");
	console.log("file.txt written to externalFs");

	await internalFs.promises.writeFile(join(internalDir, "/file2.txt"), "hello world 2", "utf8");
	await sleep(500);
	assert.strictEqual(externalFS.fs.readFileSync(join(externalDir, "/file2.txt"), "utf8"), "hello world 2");
	console.log("file2.txt written to externalFs");

	await internalFs.promises.writeFile(join(internalDir, "/file.txt"), "hello world 3", "utf8");
	await sleep(500);
	assert.strictEqual(externalFS.fs.readFileSync(join(externalDir, "/file.txt"), "utf8"), "hello world 3");
	console.log("updates in file.txt reflected in externalFs");

	await externalFS.fs.promises.writeFile(join(externalDir, "/file2.txt"), "hello world 4");
	await sleep(500);
	assert.strictEqual(await internalFs.promises.readFile(join(internalDir, "/file2.txt"), "utf8"), "hello world 4");
	console.log("updates in file2.txt reflected in internalFs");

	await internalFs.promises.rm(join(internalDir, "/file.txt"), {});
	console.log("file.txt removed from internalFs");
	await sleep(500);
	const externalReadDir = externalFS.fs.readdirSync(externalDir);
	assert.deepStrictEqual(externalReadDir, ["file2.txt"]);
	console.log("file.txt removed from externalFs");

	await externalFS.fs.promises.rm(join(externalDir, "/file2.txt"));
	console.log("file2.txt removed from externalFs");
	await sleep(500);
	const internalReadDir = await internalFs.promises.readdir(internalDir);
	assert.deepStrictEqual(internalReadDir, []);
	console.log("file2.txt removed from internalFs");
}

async function after() {
	cleanup();
	internalFs.unwatchFile("*");
	externalFS.fs.unwatchFile(externalDir);
	externalFS.vol.reset();
}

before()
	.then(main)
	.then(after)
	.then(() => {
		console.log("done.");
		process.exit(0);
	})
	.catch((e) => {
		console.error(e);
		process.exit(1);
	});

setTimeout(() => {
	console.error("timed out.");
	process.exit(1);
}, +require("../.mocharc.json").timeout);
