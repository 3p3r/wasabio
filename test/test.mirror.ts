import * as wasabio from "../dist";
import externalFs from "memfs";
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
	externalFs.vol.reset();
	cleanup = await internalFs.mirror(
		externalFs.fs as any,
		externalDir,
		internalDir,
		(f) => f.endsWith(".txt"),
		console.log,
	);
}

async function main() {
	console.log("mirror utility tests");
	await internalFs.promises.writeFile(join(internalDir, "/file.txt"), "hello world 1", "utf8");
	await sleep(500);
	assert.strictEqual(externalFs.fs.readFileSync(join(externalDir, "/file.txt"), "utf8"), "hello world 1");
	console.log("file.txt written to externalFs");
	await internalFs.promises.writeFile(join(internalDir, "/file2.txt"), "hello world 2", "utf8");
	await sleep(500);
	assert.strictEqual(externalFs.fs.readFileSync(join(externalDir, "/file2.txt"), "utf8"), "hello world 2");
	console.log("file2.txt written to externalFs");
	await internalFs.promises.rm(join(internalDir, "/file.txt"), {});
	console.log("file.txt removed from internalFs");
	await sleep(500);
	const externalReadDir = externalFs.fs.readdirSync(externalDir);
	assert.deepStrictEqual(externalReadDir, ["file2.txt"]);
	await externalFs.fs.promises.rm(join(externalDir, "/file2.txt"));
	console.log("file2.txt removed from externalFs");
	await sleep(500);
	const internalReadDir = await internalFs.promises.readdir(internalDir);
	assert.deepStrictEqual(internalReadDir, []);
}

async function after() {
	cleanup();
	internalFs.unwatchFile("*");
	externalFs.fs.unwatchFile(externalDir);
	externalFs.vol.reset();
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
