# ![wasabio](wasabio.png)

WebAssembly and SharedArrayBuffer IO. Pronounced "wassabee-yo".

## purpose

`wasabio` offers several utility APIs with familiar interfaces, but implemented
to work over a single SharedArrayBuffer and inside a TS-wrapped WASM module.

`wasabio` provides infrastructure for multi threaded applications running in web
browsers, enabling safe cross-worker communication and shared state.

Currently `wasabio` provides:

- Node's FS API (implemented on top of an in-memory POSIX filesystem)
- LocalStorage API (implemented on top of an in-memory key-value store)
- Node's EventEmitter API (implemented on top of a SharedArrayBuffer)
- Low level Locks, Buses, and various other utilities
