# ![wasabio](wasabio.png)

WebAssembly and SharedArrayBuffer IO. Pronounced "wassabee-yo".

## Purpose

`wasabio` offers several utility APIs with familiar interfaces, and implemented
to work over a single SharedArrayBuffer and inside a TS-wrapped WASM module.

`wasabio` provides infrastructure for multi threaded applications running in web
browsers, enabling safe cross-worker communication and shared state.

`wasabio` can be used in bundlers like Webpack to allow for an exchange between
node core modules and its own implementations to seamlessly run in the browser.

Currently `wasabio` provides:

- Node's FileSystem API (implemented on top of an in-memory POSIX filesystem)
- LocalStorage API (implemented on top of an in-memory key-value store)
- Node's EventEmitter API (implemented on top of a SharedArrayBuffer)
- Low level Locks, Buses, Channels, and various other utilities

## Usage

### API

All APIs are spec-compatible with their original counterparts.

> **note:** library needs to be initialized before use, see below.

```typescript
import {
	localStorage, // in-memory key-value store for cross worker state storage
	EventEmitter, // lets you make named emitter for cross worker communication
	readFile,
	writeFile,
	appendFile,
	readFileSync,
	writeFileSync,
	appendFileSync,
	// ... Node FS api
} from "wasabio";
```

Some utility functions are also provided:

```typescript
// return true if library is initialized
available(): boolean;
// serializes memory to a buffer, sets the thread counter to 0
serialize(memory: WebAssembly.Memory): Uint8Array;
// deserializes buffer to memory, sets the correct buffer size
deserialize(buffer: Uint8Array): WebAssembly.Memory;
```

### Initialization

#### From New Memory

Initialize `wasabio` on the first thread (usually the main thread) like so:

```typescript
// main.ts
import { initialize } from "wasabio";
const mem = await initialize();
// some time later
worker.postMessage(mem);
```

On other threads (workers, frames, etc.) initialize `wasabio` like so:

```typescript
// worker.ts
import { initialize } from "wasabio";
addEventListener("message", async ({ data }) => {
	initialize(data, { sync: true }); // sync as an option
});
```

#### From Existing Memory

Initialize `wasabio` on the first thread (usually the main thread) like so:

```typescript
// main.ts
import { initialize } from "wasabio";
const cold = getSharedArrayBufferFromSomewhere();
const hot = await initialize(cold, { reboot: true });
// some time later
worker.postMessage(hot);
```

On other threads (workers, frames, etc.) initialize `wasabio` like so:

```typescript
// worker.ts
import { initialize } from "wasabio";
addEventListener("message", async ({ data }) => {
	initialize(data, { reboot: false });
});
```

In this case, `reboot` signifies that the library is being initialized from cold
storage and thread-local state should be reset.
