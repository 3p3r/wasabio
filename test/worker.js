import * as wasabio from "../dist";
const { initialize, EventEmitter, mkdirSync } = wasabio || globalThis.WASABIO;
globalThis.onmessage = ({ data }) => {
	const { test, mem } = data;
	initialize(mem, { sync: true });
	if (test === "events") {
		const emitter = new EventEmitter("worker");
		emitter.emit("test", "hello");
	}
	if (test === "fs") {
		mkdirSync("/test3");
	}
};
