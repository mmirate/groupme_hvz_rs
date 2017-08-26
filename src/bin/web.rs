#[macro_use] extern crate clap;
extern crate susanoo;

use susanoo::hyper;
use susanoo::prelude::*;
use susanoo::response::Redirect;

fn hello(ctx: Context) -> Outcome<Response> {
    ctx.respond(Redirect::temporary("https://gfycat.com/BrightDecentBrocketdeer"))
}

fn ping(ctx: Context) -> Outcome<Response> { ctx.respond("It may be working...") }

fn welcome(ctx: Context) -> Outcome<Response> {
    ctx.respond(Response::new().with_header(hyper::header::ContentType::html()).with_body(r#"<h1>Deployment successful</h1><h1>Next step</h1><ol><li>Setup <a href="https://uptimerobot.com" target="_blank">Uptime Robot</a> to monitor <a href="/ping">this page</a> every less-than-15 minutes.</li></ol>"#))
}

fn get_server_port() -> u16 { std::env::var("PORT").unwrap_or(String::new()).parse().unwrap_or(8080) }

fn main() {
    let _matches = clap::App::new("HvZ/GroupMe interactor's web page server").version(crate_version!()).author(crate_authors!()).get_matches();

    Susanoo::default()
        .with_route(Route::get("/", hello))
        .with_route(Route::get("/ping", ping))
        .with_route(Route::get("/welcome", welcome))
        .bind(([0,0,0,0], get_server_port())).run()
}
