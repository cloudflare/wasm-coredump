// Copyright (c) 2023 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

use coredump_to_stack::CoredumpToStack;
use std::collections::HashMap;
use worker::*;

mod sentry;

fn files(form: &worker::FormData, name: &str) -> Result<Vec<worker::File>> {
    let res = form.get_all(name).ok_or(Error::JsError(format!(
        "`{name}` files is missing, please specify a {name}."
    )))?;

    let mut files = vec![];

    for res in res {
        if let worker::FormEntry::File(v) = res {
            files.push(v)
        } else {
            return Err(Error::JsError(format!("expected `{name}` to be a file")));
        }
    }

    Ok(files)
}

fn file(form: &worker::FormData, name: &str) -> Result<worker::File> {
    let res = form.get(name).ok_or(Error::JsError(format!(
        "`{name}` file is missing, please specify a {name}."
    )))?;
    if let worker::FormEntry::File(v) = res {
        Ok(v)
    } else {
        Err(Error::JsError(format!("expected `{name}` to be a file")))
    }
}

fn field(form: &worker::FormData, name: &str) -> Result<String> {
    let res = form.get(name).ok_or(Error::JsError(format!(
        "`{name}` file is missing, please specify a {name}."
    )))?;
    if let worker::FormEntry::Field(v) = res {
        Ok(v)
    } else {
        Err(Error::JsError(format!("expected `{name}` to be a file")))
    }
}

#[event(start)]
fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[event(fetch)]
async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let bucket = if let Ok(b) = env.bucket("STORAGE") {
        Some(b)
    } else {
        None
    };

    let sentry = if let Ok(_) = env.var("SENTRY_HOST") {
        let sentry = sentry::Sentry::from_env(&env)?;
        Some(sentry)
    } else {
        None
    };

    let data = req.form_data().await?;

    let eyeball_request = field(&data, "request")?;
    let eyeball_request = serde_json::from_str(&eyeball_request)
        .map_err(|err| Error::JsError(format!("failed to parse eyeball request: {err}")))?;

    let coredump = file(&data, "coredump")?;
    let coredump = coredump.bytes().await?;

    let mut sentry_tags = HashMap::new();

    // The debug_module is a Wasm Module that contains the debugging information
    // as custom sections. Either it's sent by the wasm-coredump-js client or
    // sections have been splitted out locally and stored in R2.
    let coredump_to_stack = {
        if let Some(build_id) = files(&data, "build_id-section")?.first() {
            // A build_id section indicates that the module has been splitted
            // off and we need to fetch the debug_module from R2.
            let build_id_section = build_id.bytes().await?;

            let build_id =
                wasm_parser::parse_custom_section_build_id(&build_id_section).map_err(|err| {
                    Error::JsError(format!("failed to parse build_id section: {err}"))
                })?;
            let build_id = uuid::Uuid::from_slice(&build_id)
                .map_err(|err| Error::JsError(format!("failed to parse build_id value: {err}")))?;

            let bucket = bucket
                .as_ref()
                .ok_or(Error::BindingError("missing R2 bucket".to_owned()))?;

            let key = format!("debug-{}.wasm", build_id);
            worker::console_log!("retrieve debugging information {}", key);

            sentry_tags.insert("debuginfo", key.clone());

            let debug_module = bucket
                .get(key.clone())
                .execute()
                .await?
                .expect("R2 is missing debugging information");
            let debug_module = debug_module.body().unwrap().bytes().await?;

            CoredumpToStack::new(&coredump)
                .map_err(|err| Error::JsError(format!("coredump_to_stack failed: {err}")))?
                .with_debug_module(&debug_module)
                .map_err(|err| Error::JsError(format!("with_debug_module failed: {err}")))?
        } else {
            let sections_sent_by_js_client = [
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

            let mut sections = HashMap::new();

            for section_name in sections_sent_by_js_client {
                let bytes = file(&data, &format!("{}-section", section_name))?
                    .bytes()
                    .await?;
                sections.insert(section_name, bytes);
            }

            CoredumpToStack::new(&coredump)
                .map_err(|err| Error::JsError(format!("coredump_to_stack failed: {err}")))?
                .with_debug_sections(sections)
                .map_err(|err| Error::JsError(format!("with_debug_module failed: {err}")))?
        }
    };

    let stack_frames = coredump_to_stack
        .stack()
        .map_err(|err| Error::JsError(format!("coredump_to_stack failed: {err}")))?;

    let now = Date::now();
    let key = format!("coredump.{}", now.as_millis());

    if let Some(bucket) = &bucket {
        bucket.put(key.clone(), coredump).execute().await?;
    }

    worker::console_log!("core dumped: {}", key);
    sentry_tags.insert("file", key.clone());

    // Print stack trace to console
    {
        worker::console_log!("Error: Wasm trapped.");

        for i in (0..stack_frames.len()).rev() {
            let frame = &stack_frames[i];
            worker::console_log!(
                "    at {} ({}:{})",
                frame.name,
                frame.location.file,
                frame.location.line
            );
        }
    }

    if let Some(sentry) = &sentry {
        let mut sentry_frames = vec![];

        for frame in stack_frames {
            sentry_frames.push(sentry::SentryFrame {
                function: frame.name,
                in_app: is_in_app(&frame.location.file),
                filename: frame.location.file,
                lineno: frame.location.line,
            });
        }

        sentry
            .report_exception(sentry_tags, sentry_frames, eyeball_request)
            .await?;
    }

    Response::ok(format!("{{\"key\": \"{}\"}}", key))
}

fn is_in_app(path: &str) -> bool {
    !path.starts_with("/") && !path.starts_with("library/")
}
