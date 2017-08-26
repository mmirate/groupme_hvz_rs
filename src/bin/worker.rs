extern crate ctrlc;
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
    let (factiongroupname, cncgroupname) = (std::env::var("FACTION_GROUP_NAME")?.to_string(), std::env::var("CNC_GROUP_NAME")?.to_string());
    let (mut factiongroup, mut cncgroup) = (None, None);
    {
        let me = groupme::User::get()?;
        let mut allgroups = groupme::Group::list()?;
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
            let g = groupme::Group::create(cncgroupname, None, None, Some(false))?;
            groupme_hvz_rs::groupme::Recipient::post(&g, "<> this group is for command+control over your copy of the bots. do not invite anyone else here.".to_owned(), None)?;
            g
        }));
    }
    let (factiongroup, cncgroup) = (factiongroup.expect("Cannot find faction Group"), cncgroup.expect("Cannot create CnC Group"));
    let (username, password) = (std::env::var("GATECH_USERNAME")?, std::env::var("GATECH_PASSWORD")?);
    let mut conduits : Vec<Box<periodic::Periodic>> = vec![
        Box::new(conduit_to_groupme::ConduitHvZToGroupme::new(factiongroup, cncgroup, username.to_owned(), password.to_owned())?),
        //Box::new(conduit_to_hvz::ConduitGroupmeToHvZ::new(factiongroup))
    ];
    let pair = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    {
        let pair2 = pair.clone();
        ctrlc::set_handler(move || { let &(ref lock, ref cvar) = &*pair2; *(lock.lock().unwrap()) = true; cvar.notify_all(); }).expect("Error setting Ctrl-C handler");
    }
    //let signal = chan_signal::notify(&[chan_signal::Signal::TERM, chan_signal::Signal::INT, chan_signal::Signal::HUP]);
    
    let &(ref lock, ref cvar) = &*pair;
    println!("Alive!");
    let mut i = 0;
    loop {
        let mut started = lock.lock().unwrap();
        for c in conduits.iter_mut() {
            if let Err(e) = c.tick(i) {
                std::io::stderr().write(format!("\x07ERROR: {}", e).as_bytes())?;
                for e in e.iter().skip(1) {
                    std::io::stderr().write(format!("caused by: {}", e).as_bytes())?;
                }
                if let Some(backtrace) = e.backtrace() {
                    std::io::stderr().write(format!("backtrace: {:?}", backtrace).as_bytes())?;
                }
                if let Error(ErrorKind::GaTechCreds, _) = e {
                    std::io::stderr().write(format!("Please fix this problem before continuing.").as_bytes())?;
                    return Err(e);
                } else {
                    std::io::stderr().write(format!("If you see this error repeatedly, please fix it.").as_bytes())?;
                }
            };
        }
        i += 1;
        i %= 1<<15;
        println!("Tick.");
        if *(cvar.wait_timeout(started, std::time::Duration::from_secs(10)).map_err(|_| ErrorKind::SignalHandlingThreadPanicked)?.0) == true { return Ok(()) }
    }
}
