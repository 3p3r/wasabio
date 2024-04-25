import karma from "karma";
import puppeteer from "puppeteer";
import mochaConfig from "./.mocharc.json";
process.env.CHROME_BIN = puppeteer.executablePath();
module.exports = function (config: karma.Config) {
	const testPattern = "test/**/*.test.ts";
	const testHeaders = {
		"Cross-Origin-Opener-Policy": "same-origin",
		"Cross-Origin-Embedder-Policy": "require-corp",
		"Cross-Origin-Resource-Policy": "cross-origin",
		"Access-Control-Allow-Origin": "*",
		"Access-Control-Allow-Headers": "*",
		"Access-Control-Allow-Methods": "*",
	};
	config.set({
		basePath: ".",
		customHeaders: [
			...Object.entries(testHeaders).map(([name, value]) => ({
				match: ".*",
				name,
				value,
			})),
		],
		files: [
			{
				pattern: testPattern,
				included: true,
				watched: false,
				served: true,
				type: "js",
			},
		],
		client: {
			// @ts-ignore
			mocha: {
				ui: mochaConfig.ui,
				timeout: mochaConfig.timeout,
			},
		},
		concurrency: 1,
		autoWatch: false,
		singleRun: true,
		logLevel: config.LOG_WARN,
		frameworks: ["mocha", "webpack"],
		browsers: ["ChromeHeadless"],
		webpackMiddleware: {
			/* empty */
		},
		webpack: {
			resolve: {
				extensions: [".ts", ".tsx", ".js", ".tsx", ".json", ".mjs", ".cjs"],
				modules: ["node_modules"],
				// @ts-ignore
				fallback: {
					path: require.resolve("path-browserify"),
				},
			},
			module: {
				rules: [
					{
						test: /\.worker\.[tj]s?$/,
						loader: "worker-loader",
						options: {
							inline: "no-fallback",
						},
					},
					{
						test: /\.test\.[tj]s?$/,
						loader: "string-replace-loader",
						options: {
							search: 'Worker("./test/test.worker.js", { type: "module" })',
							replace: '(require("./test.worker").default)()',
						},
					},
					{
						test: /\.c?[tj]sx?$/i,
						exclude: /(node_modules)/,
						loader: "esbuild-loader",
					},
				],
			},
		},
		preprocessors: {
			[testPattern]: ["webpack"],
		},
	});
};
