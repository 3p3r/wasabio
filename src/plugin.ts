import { Type, type Static } from "@sinclair/typebox";
import { type Compiler, sources } from "webpack";
import { Glob, type GlobOptions } from "glob";
import { validate } from "schema-utils";
import assert from "assert";
import path from "path";
import fs from "fs";

import * as wasabio from ".";

const schema = Type.Object({
	name: Type.String(),
	include: Type.Array(Type.String()),
	baseDir: Type.Optional(Type.String()),
	options: Type.Optional(Type.Object(Type.Any())),
});

export type WasabioPluginOptions = Static<typeof schema> & {
	options?: GlobOptions | undefined;
};

const PLUGIN_NAME = "WebpackWasabioPlugin";

export default class WebpackWasabioPlugin {
	constructor(public readonly options: WasabioPluginOptions) {
		validate(schema, options, {
			name: PLUGIN_NAME,
			baseDataPath: "options",
		});
	}
	apply(compiler: Compiler): void {
		compiler.hooks.afterCompile.tapPromise(PLUGIN_NAME, async (compilation) => {
			const { baseDir, include } = this.options;
			const _baseDir = baseDir || process.cwd();
			const mem = await wasabio.initialize();
			for (const glob of include) {
				const g = new Glob(glob, {
					...this.options.options,
					withFileTypes: true,
					cwd: _baseDir,
				});
				for await (const p of g) {
					assert(typeof p !== "string");
					if (p.isDirectory()) {
						await wasabio.promises.mkdir(p.fullpath(), { recursive: true });
					} else {
						await wasabio.promises.mkdir(path.dirname(p.fullpath()), { recursive: true });
						await wasabio.promises.writeFile(p.fullpath(), await fs.promises.readFile(p.fullpath()));
					}
				}
			}
			const buf = wasabio.serialize(mem);
			const zip = await wasabio.compress(buf);
			compilation.assets[this.options.name] = new sources.RawSource(Buffer.from(zip), false);
		});
	}
}
