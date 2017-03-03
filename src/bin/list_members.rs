#[macro_use] extern crate lazy_static;
extern crate rustc_serialize;
#[macro_use] extern crate clap;
extern crate regex;
extern crate groupme_hvz_rs;
use groupme_hvz_rs::*;
use groupme_hvz_rs::errors::*;

#[macro_use] extern crate error_chain;

quick_main!(|| -> Result<()> {
    let matches = clap::App::new("GroupMe group membership lister").version(crate_version!()).author(crate_authors!())
        .arg(clap::Arg::with_name("rust")
             .short("r")
             .long("rust")
             .help("Format output as entries of a Vec<user_id>"))
        .arg(clap::Arg::with_name("GROUPID")
             .required(true)
             .index(1))
        .get_matches();
    let mut members = try!(groupme::Group::get(matches.value_of("GROUPID").unwrap())).members.clone();
    members.sort_by(|a, b| a.nickname.cmp(&b.nickname));
    for m in members {
        if matches.is_present("rust") {
            println!("{:?} /* {} */,", m.user_id, m.canonical_name());
        } else {
            println!("{}", m.canonical_name());
        }
    }
    Ok(())
});

