use worker::{event, Env, Request, Response, Result};

#[derive(serde::Deserialize)]
struct Payload {
    value: usize,
}

#[event(fetch, respond_with_errors)]
pub async fn fetch(mut req: Request, _env: Env, _ctx: worker::Context) -> Result<Response> {
    let payload = req.json::<Payload>().await?;

    let thing = MyThing {
        value: payload.value,
    };
    process_thing(&thing);
    Response::ok("ok")
}

struct MyThing {
    value: usize,
}

#[inline(never)]
fn process_thing(thing: &MyThing) {
    let result = calculate(thing.value);
    worker::console_log!("result is {}", result);
}

#[inline(never)]
fn calculate(_value: usize) -> usize {
    panic!("oops")
}
