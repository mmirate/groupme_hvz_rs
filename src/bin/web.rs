#[macro_use] extern crate clap;
extern crate pencil;

use pencil::{Pencil, Request, Response, PencilResult};

fn hello(_r: &mut Request) -> PencilResult {
    pencil::helpers::redirect("https://gfycat.com/BrightDecentBrocketdeer", 307)
}

fn ping(_r: &mut Request) -> PencilResult { Ok(Response::from("It may be working...")) }

fn welcome(_r: &mut Request) -> PencilResult {
    Ok(Response::from(r#"<h1>Deployment successful</h1><h1>Next step</h1><ol><li>Setup <a href="https://uptimerobot.com" target="_blank">Uptime Robot</a> to monitor <a href="/ping">this page</a> every less-than-15 minutes.</li></ol>"#))
}

fn get_server_port() -> u16 { std::env::var("PORT").unwrap_or(String::new()).parse().unwrap_or(8080) }

fn main() {
    let _matches = clap::App::new("HvZ/GroupMe interactor's web page server").version(crate_version!()).author(crate_authors!()).get_matches();
    let mut app = Pencil::new("/");
    app.get("/", "hello", hello);
    app.get("/ping", "ping", ping);
    app.get("/welcome", "welcome", welcome);
    app.run(("0.0.0.0", get_server_port()));
}
