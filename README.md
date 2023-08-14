# Wasm Coredump Service

Link to blog post: ...

## Usage

The JS library `@cloudflare/wasm-coredump` is used to send Coredump to the Wasm
Coredump Service.
Install the library:

```
yarn add `@cloudflare/wasm-coredump`
```

Change your Worker's entrypoint to catch crashes and extract a coredump (if any):

`./src/entry.mjs`:
```js
import shim, { getMemory, wasmModule } from "../build/worker/shim.mjs"

async function fetch(request, env, ctx) {
    try {
        return shim.fetch(request, env, ctx);
    } catch (err) {
      const memory = getMemory();
      const coredumpService = env.COREDUMP_SERVICE;
      await recordCoredump({ memory, wasmModule, request, coredumpService });
      throw err;
    }
}

export default { fetch };
```

Point wrangler to the new Worker entrypoint:

`wrangler.toml`:
```
...
main = "src/entry.mjs"
...
```

Modify the wrangler.toml build command to add core dump support:
```toml
[build]
command = "cargo install worker-build && COREDUMP=1 worker-build --dev"
```

Now, when a Worker (using the Wasm Coredump Service) crashes you should see in the
logs:

```
...
POST https://example.com/make-a-crash - Ok @ 7/24/2023, 12:15:01 PM
...
  (log) retrieve debugging information debug-4dbd0d41-63aa-4a4f-be75-296f2f10b53f.wasm
  (log) core dumped: coredump.1691398686241
  (log) Error: Wasm trapped.
  (log)     at panic_abort::__rust_start_panic (library/panic_abort/src/lib.rs:32)
  (log)     at panicking::rust_panic (library/std/src/panicking.rs:740)
  (log)     at panicking::rust_panic_with_hook (library/std/src/panicking.rs:652)
  (log)     at begin_panic_handler::{closure#0} (library/std/src/panicking.rs:578)
  (log)     at backtrace::__rust_end_short_backtrace<std::panicking::begin_panic_handler::{closure_env#0}, !> (library/std/src/sys_common/backtrace.rs:146)
  (log)     at panicking::begin_panic_handler (library/std/src/panicking.rs:526)
  (log)     at panicking::panic_fmt (library/core/src/panicking.rs:52)
  (log)     at test_coredump_worker::calculate (src/lib.rs:30)
  (log)     at test_coredump_worker::process_thing (src/lib.rs:24)
  (log)     at {closure#0}::{async_block#0}<test_coredump_worker::_worker_fetch::_::__wasm_bindgen_generated_fetch::{async_block_env#0}> (/home/sven/.cargo/registry/src/index.crates.io-6f17d22bba15001f/wasm-bindgen-futures-0.4.34/src/lib.rs:218)
  (log)     at Task::run (src/task/singlethread.rs:84)
  (log)     at new::{closure#0} (src/queue.rs:81)
  (log)     at describe::invoke<wasm_bindgen::JsValue, ()> (/home/sven/.cargo/registry/src/index.crates.io-6f17d22bba15001f/wasm-bindgen-0.2.84/src/closure.rs:620)
  (log)     at memcpy::memcpy (src/macros.rs:305)
  (log)     at wasm_bindgen_futures::future_to_promise<test_coredump_worker::_worker_fetch::_::__wasm_bindgen_generated_fetch::{async_block_env#0}> (/home/sven/.cargo/registry/src/index.crates.io-6f17d22bba15001f/wasm-bindgen-futures-0.4.34/src/lib.rs:209)
  (log)     at _::__wasm_bindgen_generated_fetch (src/lib.rs:8)
  (log) reported to Sentry: "75e52055-9d4a-4bfc-9490-b4b66468e06c"
...
```

Additionally you can configure the Wasm Coredump Service to report the exception
in Sentry and/or store the coredump file in R2.


## Sending errors to Sentry

Add to the `wrangler.toml`:

```
...
[vars]
SENTRY_HOST = "..."
SENTRY_PROJECT_ID = "..."
SENTRY_API_KEY = "..."
SENTRY_CF_ACCESS_CLIENT_ID = "..."
SENTRY_CF_ACCESS_CLIENT_SECRET = "..."
```

## Store coredumps in R2

Add to the `wrangler.toml`:

```
[[r2_buckets]]
binding = 'STORAGE'
bucket_name = '...'
```

Note that the binding name has to be set to `STORAGE`.

[Wasm Coredump]: https://github.com/WebAssembly/tool-conventions/blob/main/Coredump.md
