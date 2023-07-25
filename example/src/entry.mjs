import shim, { getMemory, wasmModule } from "../build/worker/shim.mjs"
import { recordCoredump } from "@cloudflare/wasm-coredump"

async function fetch(request, env, ctx) {
    try {
        return await shim.fetch(request, env, ctx);
    } catch (err) {
      const memory = getMemory();
      const coredumpService = env.COREDUMP_SERVICE;
      await recordCoredump({ memory, wasmModule, request, coredumpService });
      throw err;
    }
}

export default { fetch };
