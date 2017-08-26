extern crate clap;
extern crate groupme_hvz_rs;
use groupme_hvz_rs::*;
use groupme_hvz_rs::errors::*;

#[macro_use] extern crate error_chain;

quick_main!(|| -> Result<()> {
    Ok(std::fs::File::create("/tmp/annx.png")?.write_all(render::render("Two more things. (1) If you start your message with \"@Human Chat\" or \"@General Chat\" while I'm still alive, I'll repost it to the requested HvZ website chat. (2) If your message includes the two words \"I'm dead\" adjacently and in that order, but without the doublequotes and regardless of capitalization or non-doublequote punctuation ... I will kick you from the Group within a few seconds.".to_owned())?.as_slice())?)
});
