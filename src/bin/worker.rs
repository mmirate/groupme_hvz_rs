extern crate rustc_serialize;
#[macro_use] extern crate chan;
extern crate chan_signal;
#[macro_use] extern crate clap;
extern crate groupme_hvz_rs;
#[macro_use] extern crate error_chain;
use groupme_hvz_rs::*;
use groupme_hvz_rs::errors::*;

use std::io::Write;

quick_main!(run);

fn run() -> Result<()> {
    let _matches = clap::App::new("HvZ/GroupMe interactor").version(crate_version!()).author(crate_authors!())
        //.arg(clap::Arg::with_name("FACTION_GROUP_ID")
        //     .required(true)
        //     .index(1))
        //.arg(clap::Arg::with_name("CNC_GROUP_ID")
        //     .required(true)
        //     .index(2))
        //.arg(clap::Arg::with_name("GATECH_USERNAME")
        //     .required(true)
        //     .index(3))
        //.arg(clap::Arg::with_name("GATECH_PASSWORD")
        //     .required(true)
        //     .index(4))
        .get_matches();
    let (factiongroupname, cncgroupname) = (try!(std::env::var("FACTION_GROUP_NAME")).to_string(), try!(std::env::var("CNC_GROUP_NAME")).to_string());
    let (mut factiongroup, mut cncgroup) = (None, None);
    {
        let me = try!(groupme::User::get());
        let mut allgroups = try!(groupme::Group::list());
        allgroups.sort_by(|a, b| a.members.len().cmp(&b.members.len()));
        while let Some(g) = allgroups.pop() {
            if factiongroup.is_none() && g.name == factiongroupname && (g.members.len() > 2 || g.creator_user_id == me.user_id) {
                std::mem::replace(&mut factiongroup, Some(g));
            } else if cncgroup.is_none() && g.name == cncgroupname && g.members.len() == 1 && g.creator_user_id == me.user_id {
                std::mem::replace(&mut cncgroup, Some(g));
            } else if factiongroup.is_some() && cncgroup.is_some() { break; }
        }
    }
    if cncgroup.is_none() {
        println!("Upserting CnC Group.");
        std::mem::replace(&mut cncgroup, Some({
            let g = try!(groupme::Group::create(cncgroupname, None, None, Some(false)));
            try!(groupme_hvz_rs::groupme::Recipient::post(&g, "<> this group is for command+control over your copy of the bots. do not invite anyone else here.".to_owned(), None));
            g
        }));
    }
    let (factiongroup, cncgroup) = (factiongroup.expect("Cannot find faction Group"), cncgroup.expect("Cannot create CnC Group"));
    let (username, password) = (try!(std::env::var("GATECH_USERNAME")), try!(std::env::var("GATECH_PASSWORD")));
    let mut conduits : Vec<Box<periodic::Periodic>> = vec![
        Box::new(try!(conduit_to_groupme::ConduitHvZToGroupme::new(factiongroup, cncgroup, username.to_owned(), password.to_owned()))),
        //Box::new(conduit_to_hvz::ConduitGroupmeToHvZ::new(factiongroup))
    ];
    let signal = chan_signal::notify(&[chan_signal::Signal::TERM, chan_signal::Signal::INT, chan_signal::Signal::HUP]);
    println!("Alive!");
    let mut i = 0;
    loop {
        for c in conduits.iter_mut() {
            if let Err(e) = c.tick(i) {
                try!(std::io::stderr().write(format!("\x07ERROR: {}", e).as_bytes()));
                for e in e.iter().skip(1) {
                    try!(std::io::stderr().write(format!("caused by: {}", e).as_bytes()));
                }
                if let Some(backtrace) = e.backtrace() {
                    try!(std::io::stderr().write(format!("backtrace: {:?}", backtrace).as_bytes()));
                }
                if let Error(ErrorKind::GaTechCreds, _) = e {
                    try!(std::io::stderr().write(format!("Please fix this problem before continuing.").as_bytes()));
                    return Err(e);
                } else {
                    try!(std::io::stderr().write(format!("If you see this error repeatedly, please fix it.").as_bytes()));
                }
            };
        }
        i += 1;
        i %= 1<<15;
        println!("Tick.");
        chan_select! {
            default => std::thread::sleep(std::time::Duration::new(8,0)),
            signal.recv() -> signal => {
                println!("Exiting in receipt of {:?}.", signal);
                return Ok(());
            }
        }
    }
}
