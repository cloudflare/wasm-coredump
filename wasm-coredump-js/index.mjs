// Copyright (c) 2023 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

// Call this function on Wasm exception to record the Wasm coredump, if any.
export async function recordCoredump({
  memory,
  wasmModule,
  request,
  coredumpService,
}) {
  try {
    const image = memory.buffer;

    // Check for the presence of the wasm\0 header, meaning a coredump
    // has been written.
    const u32 = new Uint32Array(image);
    if (u32[0] !== 0x6d736100) {
      return null;
    }

    const body = new FormData();
    body.set("coredump", new Blob([image]));

    // There are two possible case for providing the coredump service the
    // debugging information:
    // 1. the debugging information are inside the user Wasm module, in which
    //    case we sent the sections and the coredump sercice construct a debug
    //    module.
    // 2. the debugging information were too big and were splitted out of the
    //    user module locally. In which case we just send a build_id to the
    //    coredump section. It should be able to fetch the debugging information
    //    from R2.
    //
    //  The presence of the build_id section indicates that it's case 2.
    //  Otherwise safe to assume it's 1.
    const buildIdSections = WebAssembly.Module.customSections(
      wasmModule,
      "build_id",
    );
    if (buildIdSections.length > 0) {
      buildIdSections.forEach((section) => {
        body.append("build_id-section", new Blob([section]));
      });
    } else {
      const sectionsToExtract = [
        "name",
        ".debug_info",
        ".debug_pubtypes",
        ".debug_loc",
        ".debug_ranges",
        ".debug_abbrev",
        ".debug_line",
        ".debug_str",
        ".debug_pubnames",
      ];

      for (let i = 0, len = sectionsToExtract.length; i < len; i++) {
        const name = sectionsToExtract[i];
        const sections = WebAssembly.Module.customSections(wasmModule, name);
        sections.forEach((section) => {
          body.append(name + "-section", new Blob([section]));
        });
      }
    }

    // Add eyeball request
    {
      const headers = {};
      for (const [key, value] of request.headers.entries()) {
        headers[key] = value;
      }

      const eyeballRequest = {
        method: request.method,
        url: request.url,
        headers,
      };

      body.set("request", JSON.stringify(eyeballRequest));
    }

    const res = await coredumpService.fetch("http://coredump-service", {
      method: "POST",
      body,
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`failed to report coredump: ${res.status}, ${text}`);
    }
  } catch (err) {
    console.error("failed to get coredump: " + err);
  }
}
