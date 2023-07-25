// Copyright (c) 2023 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;
use worker::Result;

#[derive(serde::Serialize)]
pub(crate) struct SentryFrame {
    pub(crate) filename: String,
    pub(crate) function: String,
    pub(crate) lineno: u32,
    pub(crate) in_app: bool,
}

pub(crate) struct Sentry {
    url: String,
    cf_access_client_id: String,
    cf_access_client_secret: String,
}

impl Sentry {
    pub(crate) fn from_env(env: &worker::Env) -> Result<Self> {
        let sentry_host = env.var("SENTRY_HOST")?.to_string();
        let sentry_project_id = env.var("SENTRY_PROJECT_ID")?.to_string();
        let sentry_api_key = env.var("SENTRY_API_KEY")?.to_string();
        let cf_access_client_id = env.var("SENTRY_CF_ACCESS_CLIENT_ID")?.to_string();
        let cf_access_client_secret = env.secret("SENTRY_CF_ACCESS_CLIENT_SECRET")?.to_string();

        Ok(Self {
            url: format!("https://{}/api/{}/store/?sentry_version=7&sentry_client=coredump-service&sentry_key={}", sentry_host, sentry_project_id, sentry_api_key),
            cf_access_client_id,
            cf_access_client_secret,
        })
    }

    pub(crate) async fn report_exception(
        &self,
        tags: HashMap<&'static str, String>,
        frames: Vec<SentryFrame>,
        eyeball_request: serde_json::Value,
    ) -> Result<()> {
        let event_id = Uuid::new_v4().to_string();
        let now_in_seconds = worker::Date::now().as_millis() / 1000;
        let event = json!({
            "event_id": event_id,
            "timestamp": now_in_seconds,
            "platform": "rust",
            "logger": "coredump-service",
            "exception": {
                "values": [
                    {
                        "type": "Error",
                        "value": "Wasm crashed",
                        "stacktrace": {
                            "frames": frames
                        }
                    }
                ],
            },
            "request": eyeball_request,
            "level": "fatal",
            "tags": tags
        });

        self.post(event).await
    }

    async fn post(&self, data: serde_json::Value) -> Result<()> {
        let body = serde_json::to_string(&data).unwrap();

        let mut headers = worker::Headers::new();
        headers.set("Cf-Access-Client-Id", &self.cf_access_client_id)?;
        headers.set("Cf-Access-Client-Secret", &self.cf_access_client_secret)?;

        let mut init = worker::RequestInit::new();

        let req = worker::Request::new_with_init(
            &self.url,
            init.with_method(worker::Method::Post)
                .with_headers(headers)
                .with_body(Some(body.into())),
        )?;

        let mut res = worker::Fetch::Request(req).send().await?;
        if res.status_code() != 200 {
            let text = res.text().await?;
            return Err(worker::Error::RustError(format!(
                "Unexpected Sentry response {}: {}",
                res.status_code(),
                text
            )));
        }

        worker::console_log!("reported to Sentry: {}", data.get("event_id").unwrap());
        Ok(())
    }
}
