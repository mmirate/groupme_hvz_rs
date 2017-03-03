extern crate rustc_serialize;
extern crate clap;
extern crate groupme_hvz_rs;
use groupme_hvz_rs::*;
use groupme_hvz_rs::errors::*;

use std::io::Write;

fn main() {
    let _matches = clap::App::new("HvZ/GroupMe interactor").version("0.0.2").author("Milo Mirate <mmirate@gatech.edu>")
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
    let (mut factiongroup, mut cncgroup) = (None, None);
    {
        let me = groupme::User::get().unwrap();
        let groupname = |varname: &'static str| std::env::var(varname).unwrap().to_string();
        let mut allgroups = groupme::Group::list().unwrap();
        allgroups.sort_by(|a, b| a.members.len().cmp(&b.members.len()));
        while let Some(g) = allgroups.pop() {
            if factiongroup.is_none() && g.name == groupname("FACTION_GROUP_NAME") && (g.members.len() > 2 || g.creator_user_id == me.user_id) {
                std::mem::replace(&mut factiongroup, Some(g));
            } else if cncgroup.is_none() && g.name == groupname("CNC_GROUP_NAME") && g.members.len() == 1 && g.creator_user_id == me.user_id {
                std::mem::replace(&mut cncgroup, Some(g));
            } else if factiongroup.is_some() && cncgroup.is_some() { break; }
        }
    }
    if cncgroup.is_none() {
        let groupname = |varname: &'static str| std::env::var(varname).unwrap().to_string();
        println!("Upserting CnC Group.");
        std::mem::replace(&mut cncgroup, Some({
            let g = groupme::Group::create(groupname("CNC_GROUP_NAME"), None, None, Some(false)).unwrap();
            groupme_hvz_rs::groupme::Recipient::post(&g, "<> this group is for command+control over your copy of the bots. do not invite anyone else here.".to_owned(), None).unwrap();
            g
        }));
    }
    let (factiongroup, cncgroup) = (factiongroup.expect("Cannot find faction Group"), cncgroup.expect("Cannot create CnC Group"));
    let (username, password) = (std::env::var("GATECH_USERNAME").unwrap(), std::env::var("GATECH_PASSWORD").unwrap());
    let mut conduits : Vec<Box<periodic::Periodic>> = vec![
        Box::new(conduit_to_groupme::ConduitHvZToGroupme::new(factiongroup, cncgroup, username.to_owned(), password.to_owned())),
        //Box::new(conduit_to_hvz::ConduitGroupmeToHvZ::new(factiongroup))
    ];
    println!("Alive!");
    let mut i = 0;
    loop {
        for c in conduits.iter_mut() {
            if let Err(ref e) = c.tick(i) {
                std::io::stderr().write(format!("\x07ERROR: {}", e).as_bytes()).unwrap();
                for e in e.iter().skip(1) {
                    std::io::stderr().write(format!("caused by: {}", e).as_bytes()).unwrap();
                }
                if let Some(backtrace) = e.backtrace() {
                    std::io::stderr().write(format!("backtrace: {:?}", backtrace).as_bytes()).unwrap();
                }
                if let &Error(ErrorKind::GaTechCreds, _) = e {
                    std::io::stderr().write(format!("Please fix this problem before continuing.").as_bytes()).unwrap();
                    std::process::exit(1);
                } else {
                    std::io::stderr().write(format!("If you see this error repeatedly, please fix it.").as_bytes()).unwrap();
                }
            };
        }
        i += 1;
        i %= 1<<15;
        println!("==== Sleeping now. ====");
        std::thread::sleep(std::time::Duration::new(8,0));
    }
}
