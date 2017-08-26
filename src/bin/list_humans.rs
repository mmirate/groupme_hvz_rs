#[macro_use] extern crate clap;
extern crate regex;
extern crate groupme_hvz_rs;
use groupme_hvz_rs::*;
use groupme_hvz_rs::errors::*;

#[macro_use] extern crate error_chain;

quick_main!(|| -> Result<()> {
    let matches = clap::App::new("HvZ Human-faction membership lister").version(crate_version!()).author(crate_authors!())
        .arg(clap::Arg::with_name("rust")
             .short("r")
             .long("rust")
             .help("Format output as entries of a Vec<gtname>"))
        .get_matches();
    let (username, password) = (std::env::var("GATECH_USERNAME")?, std::env::var("GATECH_PASSWORD")?);
    let mut scraper = hvz::HvZScraper::new(username.to_owned(), password.to_owned());
    let mut players = scraper.fetch_killboard()?.remove(&hvz::Faction::Human).unwrap_or(vec![]);
    players.sort_by(|a, b| a.playername.cmp(&b.playername));
    for p in players {
        if matches.is_present("rust") {
            println!("{:?} /* {} */,", p.gtname, p.playername);
        } else {
            println!("{}", p.playername);
        }
    }
    Ok(())
});

