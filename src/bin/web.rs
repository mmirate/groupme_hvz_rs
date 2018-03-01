//extern crate futures;
extern crate gotham;
extern crate hyper;
extern crate mime;

use hyper::{Response, StatusCode};

use gotham::http::response::create_response;
use gotham::state::State;
use gotham::router::Router;
use gotham::router::builder::{build_simple_router, DefineSingleRoute, DrawRoutes};

fn welcome(state: State) -> (State, Response) {
    let res = create_response(&state, StatusCode::Ok, Some((String::from(r#"<h1>Deployment successful</h1><h1>Next step</h1><ol><li>Setup <a href="https://uptimerobot.com" target="_blank">Uptime Robot</a> to monitor <a href="/ping">this page</a> every less-than-15 minutes.</li></ol>"#).into_bytes(), mime::TEXT_HTML)));

    (state, res)
}

fn ping(state: State) -> (State, Response) {
    let res = create_response(&state, StatusCode::Ok, Some((String::from("Pong!").into_bytes(), mime::TEXT_HTML)));

    (state, res)
}

fn nothing_to_see_here(state: State) -> (State, Response) {
    let mut res = create_response(&state, StatusCode::TemporaryRedirect, None);
    res.headers_mut().set(hyper::header::Location::new("https://gfycat.com/BrightDecentBrocketdeer"));

    (state, res)
}

fn router() -> Router {
    build_simple_router(|route| {
        route.get("/").to(nothing_to_see_here);
        route.get("/welcome").to(welcome);
        route.get("/ping").to(ping);
    })
}

#[inline]
fn get_server_port() -> u16 {
    (|| -> Result<_, Box<std::error::Error>> { Ok(std::env::var("PORT")?.parse()?) })().unwrap_or(8080)
}

pub fn main() {
    let addr = ("0.0.0.0", get_server_port());
    println!("Listening for requests at http://{:?}", addr);
    gotham::start(addr, router())
}
