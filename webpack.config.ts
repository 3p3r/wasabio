import fs from "fs";
import path from "path";
import chalk from "chalk";
import webpack from "webpack";
import childProcess from "child_process";
import CopyPlugin from "copy-webpack-plugin";
import TerserPlugin from "terser-webpack-plugin";
import ShellPlugin from "webpack-shell-plugin-next";

const OUT_DIR = path.resolve("dist");
fs.rmSync(OUT_DIR, { recursive: true, force: true });

const $ = (cmd: string) => childProcess.execSync(cmd, { encoding: "utf-8", stdio: "pipe" }).trim();
const cd = (dir: string) => process.chdir(dir);
const within = (fn: () => void) => {
	const cwd = process.cwd();
	try {
		fn();
	} finally {
		process.chdir(cwd);
	}
};
const withWebAssemblyRust = (fn: () => void) => {
	console.log(chalk.bgBlackBright("using web assembly Rust"));
	fn();
};
const withSystemRust = (fn: () => void) => {
	within(() => {
		cd(__dirname);
		$("mv .cargo .cargo.reset");
		$("mv rust-toolchain rust-toolchain.reset");
	});
	try {
		console.log(chalk.bgBlackBright("using system Rust"));
		fn();
	} finally {
		within(() => {
			cd(__dirname);
			$("mv .cargo.reset .cargo");
			$("mv rust-toolchain.reset rust-toolchain");
		});
	}
};

function installEmccSDK() {
	if (fs.existsSync("deps/emcc-sdk")) {
		console.log(chalk.bgBlue("Emscripten SDK already installed"));
		return;
	}
	console.log(chalk.bgBlue("installing Emscripten SDK"));
	const ver = "3.1.35";
	const url = "https://github.com/emscripten-core/emsdk/archive/refs/heads/main.zip";
	$(`curl -L ${url} -o emsdk.zip`);
	$(`unzip emsdk.zip`);
	$(`mv emsdk-main deps/emcc-sdk`);
	$(`rm emsdk.zip`);
	within(() => {
		cd("deps/emcc-sdk");
		$(`./emsdk install ${ver}`);
		$(`./emsdk activate ${ver}`);
		cd("upstream/emscripten");
		$(`ln -s emcc emgcc`);
		$(`ln -s emcc.py emgcc.py`);
		$(`./emgcc --version`);
	});
}

function installWasmBindgen() {
	if (fs.existsSync("deps/wasm-bindgen/patched")) {
		console.log(chalk.bgBlue("wasm-bindgen already installed"));
		return;
	}
	console.log(chalk.bgBlue("installing wasm-bindgen"));
	const ver = "0.2.87";
	const url = `https://github.com/rustwasm/wasm-bindgen/archive/refs/tags/${ver}.zip`;
	$(`curl -L ${url} -o wasm-bindgen.zip`);
	$(`unzip wasm-bindgen.zip -d deps/wasm-bindgen`);
	$(`rm wasm-bindgen.zip`);
	$(`mv deps/wasm-bindgen/wasm-bindgen-${ver} deps/wasm-bindgen/patched`);
	within(() => {
		cd("deps/wasm-bindgen/patched");
		$(`patch -p1 -i ../../../patches/wbg-tls-allocator.patch`);
		withWebAssemblyRust(() => {
			console.log(`version: ${$(`rustc --version`)}`);
		});
		withSystemRust(() => {
			console.log(`version: ${$(`rustc --version`)}`);
			$(`cargo build --release`);
			cd("crates/cli");
			$(`cargo build --release`);
		});
	});
}

function installWasiSDK() {
	if (fs.existsSync("deps/wasi-sdk")) {
		console.log(chalk.bgBlue("WASI SDK already installed"));
		return;
	}
	console.log(chalk.bgBlue("installing WASI SDK"));
	const url = "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-21/wasi-sdk-21.0-linux.tar.gz";
	$(`curl -L ${url} -o wasi-sdk.tar.gz`);
	$(`tar xf wasi-sdk.tar.gz`);
	$(`rm wasi-sdk.tar.gz`);
	$(`mv wasi-sdk-21.0 deps/wasi-sdk`);
	within(() => {
		cd("deps/wasi-sdk");
		$(`./bin/clang --version`);
	});
}

function buildWithWasmPack(webpackMode: string) {
	console.log(chalk.bgBlackBright("building with wasm-pack"));
	const rustMode = webpackMode === "production" ? "release" : "debug";
	const wbgDir = "deps/wasm-bindgen/patched/target/release";
	$(`PATH=$PATH:${wbgDir} wasm-pack build --mode no-install --target web --${rustMode}`);
	const wasmChecksum = $("md5sum pkg/wasabio_bg.wasm").split(" ")[0];
	const currentCommit = $("git rev-parse HEAD").trim();
	const pkjJson = JSON.parse(fs.readFileSync("pkg/package.json", "utf8"));
	pkjJson.checksum = wasmChecksum;
	pkjJson.commit = currentCommit;
	pkjJson.built = new Date().toISOString();
	fs.writeFileSync("pkg/package.json", JSON.stringify(pkjJson, null, 2));
}

export default function (_env: unknown, { mode }: { mode: string }) {
	const isProduction = mode === "production";

	const config: webpack.Configuration = {
		mode: isProduction ? "production" : "development",
		entry: "./src/index.ts",
		devtool: isProduction ? false : "inline-source-map",
		output: {
			path: OUT_DIR,
			library: {
				commonjs: "wasabio",
				amd: "wasabio",
				root: "WASABIO",
			},
			libraryTarget: "umd",
			umdNamedDefine: true,
			globalObject: `(typeof self !== 'undefined' ? self : this)`,
			filename: "index.js",
		},
		node: {
			global: false,
			__filename: false,
			__dirname: false,
		},
		watchOptions: {
			ignored: [OUT_DIR],
		},
		optimization: {
			nodeEnv: false,
			minimize: mode === "production",
			minimizer: [
				new TerserPlugin({
					extractComments: false,
					terserOptions: {
						format: {
							comments: false,
						},
					},
				}),
			],
		},
		performance: {
			hints: false,
		},
		plugins: [
			new ShellPlugin({
				safe: true,
				onBuildStart: {
					blocking: true,
					scripts: [
						"mkdir -p deps",
						installWasiSDK,
						installEmccSDK,
						installWasmBindgen,
						buildWithWasmPack.bind(null, mode),
					],
				},
				onAfterDone: {
					blocking: false,
					scripts: [
						[
							"npx dts-bundle-generator",
							"--export-referenced-types=false",
							"--umd-module-name=wasabio",
							"-o dist/index.d.ts",
							"src/index.ts",
						].join(" "),
					],
				},
			}),
			new webpack.ProvidePlugin({
				Buffer: ["buffer", "Buffer"],
				process: "process",
			}),
			new CopyPlugin({
				patterns: [
					{ from: "LICENSE" },
					{ from: "README.md" },
					{
						from: "package.json",
						transform: (content) => {
							const pkgJson = JSON.parse(content.toString());
							delete pkgJson.devDependencies;
							delete pkgJson.prettier;
							delete pkgJson.scripts;
							delete pkgJson.private;
							delete pkgJson.type;
							pkgJson.main = "./index.js";
							pkgJson.types = "./index.d.ts";
							return JSON.stringify(pkgJson, null, 2);
						},
					},
				],
			}),
		],
		module: {
			rules: [
				{
					test: /\.[jt]sx?$/,
					loader: "ts-loader",
					options: {
						transpileOnly: false,
					},
				},
				{
					test: /wasabio\.js$/,
					loader: "string-replace-loader",
					options: {
						search: "input = new URL('wasabio_bg.wasm', import.meta.url);",
						replace: "throw new Error('no default wasm binary bundled.');",
						strict: true,
					},
				},
				{
					test: /\.wasm$/,
					loader: "url-loader",
					options: {
						mimetype: "delete/me",
						limit: 15 * 1024 * 1024,
						// this removes the "data:<whatever>;base64," from the bundle
						generator: (content: Buffer) => content.toString("base64"),
					},
				},
			],
		},
		resolve: {
			extensions: [".tsx", ".ts", ".jsx", ".js", ".json"],
			alias: {
				assert: "assert",
				buffer: "buffer",
				process: "process",
			},
		},
	};

	return config;
}