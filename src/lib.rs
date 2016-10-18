extern crate rand;
extern crate rustc_serialize;
#[macro_use(static_slice)] extern crate static_slice;
#[macro_use] extern crate lazy_static;
pub mod groupme;
pub mod hvz;
pub mod syncer;
pub mod error;

pub mod hvz_syncer {
    use std;
    use hvz;
    use syncer;
    use error::*;
    #[derive(Debug)] pub struct HvZSyncer { pub killboard: hvz::Killboard, pub chatboard: hvz::Chatboard, pub panelboard: hvz::Panelboard, pub scraper: hvz::HvZScraper, conn: syncer::postgres::Connection, }
    pub type Changes<T> = (T, T);
    impl HvZSyncer {
        pub fn new() -> HvZSyncer {
            let mut me = HvZSyncer { scraper: hvz::HvZScraper::new(), conn: syncer::setup(), killboard: hvz::Killboard::new(), chatboard: hvz::Chatboard::new(), panelboard: hvz::Panelboard::new(), };
            std::mem::replace(&mut me .killboard, syncer::readout(&me.conn, "killboard"));
            std::mem::replace(&mut me .chatboard, syncer::readout(&me.conn, "chatboard"));
            std::mem::replace(&mut me.panelboard, syncer::readout(&me.conn, "panelboard"));
            me .killboard.entry(hvz::Faction::Human         ).or_insert(vec![]);
            me .killboard.entry(hvz::Faction::Zombie        ).or_insert(vec![]);
            me .chatboard.entry(hvz::Faction::General       ).or_insert(vec![]);
            me .chatboard.entry(hvz::Faction::Human         ).or_insert(vec![]);
            me .chatboard.entry(hvz::Faction::Zombie        ).or_insert(vec![]);
            me.panelboard.entry(hvz::PanelKind::Announcement).or_insert(vec![]);
            me.panelboard.entry(hvz::PanelKind::Mission     ).or_insert(vec![]);
            me
        }
        pub fn update_killboard(&mut self) -> ResultB<Changes<hvz::Killboard>> {
            let (killboard, additions, deletions) = try!(syncer::update_map(&self.conn, "killboard", try!(self.scraper.fetch_killboard()), &self.killboard, true));
            self.killboard = killboard;
            Ok((additions, deletions))
        }
        #[inline] pub fn new_zombies(&mut self) -> ResultB<Vec<hvz::Player>> { Ok(try!(self.update_killboard()).0.remove(&hvz::Faction::Human).unwrap_or(vec![])) }
        pub fn update_chatboard(&mut self) -> ResultB<Changes<hvz::Chatboard>> {
            let (chatboard, additions, deletions) = try!(syncer::update_map(&self.conn, "chatboard", try!(self.scraper.fetch_chatboard()), &self.chatboard, true));
            self.chatboard = chatboard;
            Ok((additions, deletions))
        }
        pub fn update_panelboard(&mut self) -> ResultB<Changes<hvz::Panelboard>> {
            let (panelboard, additions, deletions) = try!(syncer::update_map(&self.conn, "panelboard", try!(self.scraper.fetch_panelboard()), &self.panelboard, false));
            self.panelboard = panelboard;
            Ok::<Changes<hvz::Panelboard>, Box<std::error::Error>>((additions, deletions))
        }
    }
}

pub mod groupme_syncer {
    use std;
    use syncer;
    use groupme;
    use groupme::Recipient;
    use error::*;

    #[derive(Debug)] pub struct GroupmeSyncer { pub group: groupme::Group, pub last_message_id: Option<String>, pub members: Vec<groupme::Member>, conn: syncer::postgres::Connection, }
    impl GroupmeSyncer {
        pub fn new(group: groupme::Group) -> GroupmeSyncer {
            let conn = syncer::setup();
            let last_message_id = syncer::read(&conn, (group.group_id.clone() + "last_message_id").as_str()).ok();
            GroupmeSyncer { group: group, last_message_id: last_message_id, members: vec![], conn: conn }
        }
        pub fn update_messages(&mut self) -> ResultB<Vec<groupme::Message>> {
            let last_message_id = self.last_message_id.clone();
            println!("last_message_id = {:?}", &last_message_id);
            let selector = last_message_id.clone().map(groupme::MessageSelector::After);
            //let selector = match last_message_id {
            //    Some(ref m) => Some(groupme::MessageSelector::After(m.clone())),
            //    None => None,
            //};
            println!("selector = {:?}", selector);
            let ret = try!(self.group.generic_slurp_messages(selector.clone()));
            self.last_message_id = if ret.len() > 0 { Some(ret[ret.len()-1].id.clone()) } else { last_message_id };
            if let Some(ref last_message_id) = self.last_message_id { try!(syncer::write_dammit(&self.conn, (self.group.group_id.clone() + "last_message_id").as_str(), last_message_id.as_str())); }
            if let Some(_) = selector { Ok(ret) } else { Ok(vec![]) }
        }
    }

    //pub fn hijack(old_group: &mut groupme::Group) -> Result<groupme::Group, Box<std::error::Error>> {
    //    let mut new_group = try!(groupme::Group::create(old_group.name.clone(), old_group.description.clone(), old_group.image_url.clone(), Some(false)));
    //    try!(old_group.post("Oh ****! Chat boss is dead! Stand by, I'm creating a new group; invites incoming.".to_string(), None));
    //    let old_name = "~(DEFUNCT)".to_string() + &old_group.name;
    //    try!(old_group.update(Some(old_name), None, None, None));
    //    // TODO mofo'ing OFFICE MODE! where the foo is OFFICE MODE when you need it?!
    //    let new_members = old_group.members.iter().filter(|m| m.user_id != old_group.creator_user_id).cloned().collect::<Vec<groupme::Member>>();
    //    try!(new_group.add_mut(new_members));
    //    try!(new_group.post("Okay, we're up and running here. But someone else needs to run the bot, stat.".to_string(), None));
    //    let r : Result<groupme::Group, Box<std::error::Error>> = Ok(new_group);
    //    r
    //}
}

pub mod periodic {
    use error::ResultB;
    pub trait Periodic {
        fn tick(&mut self, usize) -> ResultB<()>;
    }
}

pub mod conduit_to_groupme { // A "god" object. What could go wrong?
    use std;
    use hvz;
    use hvz_syncer;
    use groupme_syncer;
    extern crate chrono;
    use self::chrono::Timelike;
    use groupme;
    use groupme::{Recipient};
    use periodic;
    use rand;
    use rand::Rng;
    //use rustc_serialize::json::ToJson;
    use std::convert::Into;
    use std::iter::FromIterator;
    use error::*;
    extern crate regex;

    fn nll(items: Vec<&str>) -> String {
        if let Some((tail, init)) = items.split_last() {
            if let Some((_, _)) = init.split_last() {
                format!("{} and {}", init.join(", "), tail.to_string())
            } else { tail.to_string() }
        } else { "".to_string() }
    }

    fn capitalize(s: &str) -> String {
        let mut c = s.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }

    #[derive(Copy, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)] enum BotRole { VoxPopuli, Chat(hvz::Faction), Killboard(hvz::Faction), Panel(hvz::PanelKind) }
    impl BotRole {
        fn nickname(&self) -> String {
            match self {
                &BotRole::VoxPopuli => "Vox Populi".to_string(),
                &BotRole::Chat(f) => capitalize(&format!("{} Chat", f)),
                &BotRole::Killboard(hvz::Faction::Zombie) => "The Messenger".to_string(),
                &BotRole::Killboard(hvz::Faction::Human) => "Fleet Sgt. Ho".to_string(),
                &BotRole::Killboard(_) => "ERROR: UNKNOWN BOT".to_string(),
                &BotRole::Panel(hvz::PanelKind::Mission) => "John from IT".to_string(),
                &BotRole::Panel(hvz::PanelKind::Announcement) => "Midnight Lantern".to_string(),
            }
        }
        fn avatar_url(&self) -> Option<String> {
            match self {
                &BotRole::VoxPopuli => "http://oyster.ignimgs.com/mediawiki/apis.ign.com/bioshock-infinite/thumb/2/2a/BillBoard_Daisy_Render.jpg/468px-BillBoard_Daisy_Render.jpg".to_string().into(),
                &BotRole::Chat(hvz::Faction::Human) => "https://upload.wikimedia.org/wikipedia/commons/thumb/1/1e/Georgia_Tech_Outline_Interlocking_logo.svg/640px-Georgia_Tech_Outline_Interlocking_logo.svg.png".to_string().into(),
                &BotRole::Chat(hvz::Faction::General) => "https://upload.wikimedia.org/wikipedia/commons/thumb/1/1e/Georgia_Tech_Outline_Interlocking_logo.svg/640px-Georgia_Tech_Outline_Interlocking_logo.svg.png".to_string().into(),
                &BotRole::Chat(hvz::Faction::Zombie) => "https://upload.wikimedia.org/wikipedia/commons/thumb/1/1e/Georgia_Tech_Outline_Interlocking_logo.svg/640px-Georgia_Tech_Outline_Interlocking_logo.svg.png".to_string().into(),
                &BotRole::Chat(_) => None,
                &BotRole::Killboard(hvz::Faction::Zombie) => "https://i.groupme.com/800x800.png.9ce096915aec40f5926ba8a8c392fa8f".to_string().into(),
                &BotRole::Killboard(hvz::Faction::Human) => "https://i.groupme.com/1280x688.jpeg.70371d1476624df0b6586d8e6d72a946".to_string().into(),
                &BotRole::Killboard(_) => None,
                &BotRole::Panel(hvz::PanelKind::Mission) => "https://i.groupme.com/250x332.jpeg.bd6de69e29b5462085e685a595be7864".to_string().into(),
                &BotRole::Panel(hvz::PanelKind::Announcement) => None,
            }
        }
        fn watdo(&self) -> String {
            match self {
                &BotRole::VoxPopuli => "If you start your message with \"@Everyone\" or \"@everyone\" while I'm still alive, I'll repost it in such a way that it will mention everyone. Abuse this, and there will be consequences.".to_string(),
                &BotRole::Chat(f) => format!("I'm the voice of {} chat. When someone posts something there, I'll tell you about it within a few seconds.{}", f, if f == hvz::Faction::General { " Except during gameplay hours, because spam is bad." } else { "" }),
                &BotRole::Killboard(hvz::Faction::Zombie) => "If someone shows up on the other side of the killboard, I'll report it here within about a minute.".to_string(),
                &BotRole::Killboard(hvz::Faction::Human) => "Whenever someone signs up, I'll report it here.".to_string(),
                &BotRole::Killboard(_) => "ERROR: UNKNOWN BOT".to_string(),
                &BotRole::Panel(hvz::PanelKind::Mission) => "If a mission arises, I'll contact you.".to_string(),
                &BotRole::Panel(hvz::PanelKind::Announcement) => "I announce the nightly announcements. #DeptOfRedundancyDept".to_string(),
            }
        }
        fn phrase(&self, param: &str) -> String {
            macro_rules! messages {
                ($($fmtstr:tt),*) => { static_slice![fn(&str) -> String: $({
                        fn f(x: &str) -> String { format!($fmtstr, x) }
                        f
                }),*] }
            }
            let formats: &[fn(&str) -> String] = match self {
                &BotRole::VoxPopuli => messages!["{}"],
                &BotRole::Chat(hvz::Faction::Zombie) => messages!["{}"],
                &BotRole::Chat(hvz::Faction::Human) => messages!["{}"],
                &BotRole::Chat(_) => messages!["{:?}"],
                &BotRole::Killboard(hvz::Faction::Zombie) => messages!["Verdammt! We lost {}. I hope it was worth it.", "(－‸ლ) {} died. Come on; we can do better than this!", "Well, I'll be. Looks like {} bit the dust.", "Well, drat. I think the zombies got {}. I hope they died with dignity.", "Well, I declare. Seems that {} kicked the bucket.", "Hunh. I guess {} turned. A grim inevitability.", "Are you kidding me? Killboard says they've nommed {}! Fight harder!", "Argh. {} passed on to the undeath."],
                &BotRole::Killboard(hvz::Faction::Human) => messages!["A warm welcome goes out to {}.", "New blood! Err, I mean, signups. Namely, {}.", "Humanity has expanded to include {}.", "Well, it seems that {} signed up for HvZ. Good luck to them."],
                &BotRole::Killboard(_) => messages!["{:?}"],
                &BotRole::Panel(hvz::PanelKind::Mission) => messages!["New mission is up! {:?}. Good luck, everyone. https://hvz.gatech.edu/missions/", "Attention! A new mission has been posted! {:?}. Let's do this! https://hvz.gatech.edu/missions/", "Mission details have been posted for {:?}. Good luck, godspeed and hail victory! https://hvz.gatech.edu/missions/"],
                &BotRole::Panel(hvz::PanelKind::Announcement) => messages!["The {:?} announcements are posted. https://hvz.gatech.edu/announcements/", "The admins have posted a {:?} announcement. https://hvz.gatech.edu/announcements/", "The {:?} announcement is up! https://hvz.gatech.edu/announcements/"],
            };
            (*rand::thread_rng().choose(formats).unwrap())(param)
        }
    }

    fn ts(m: &hvz::Message) -> String {
        let min = (chrono::Local::now() - m.timestamp).num_minutes();
        if min > 60 { m.timestamp.format(" [%a %H:%M:%S] ").to_string() }
        else if min > 2 { m.timestamp.format(" [%H:%M:%S] ").to_string() }
        else { " ".to_string() }
    }


    pub struct ConduitHvZToGroupme { factionsyncer: groupme_syncer::GroupmeSyncer, cncsyncer: groupme_syncer::GroupmeSyncer, hvz: hvz_syncer::HvZSyncer, bots: std::collections::BTreeMap<BotRole, groupme::Bot>, }
    impl ConduitHvZToGroupme {
        pub fn new(factiongroup: groupme::Group, cncgroup: groupme::Group) -> Self {
            let mut bots = std::collections::BTreeMap::new();
            for role in vec![BotRole::VoxPopuli, BotRole::Chat(hvz::Faction::Human), BotRole::Chat(hvz::Faction::General), BotRole::Killboard(hvz::Faction::Human), BotRole::Killboard(hvz::Faction::Zombie), BotRole::Panel(hvz::PanelKind::Mission), BotRole::Panel(hvz::PanelKind::Announcement)].into_iter() {
                bots.insert(role, groupme::Bot::upsert(&factiongroup, role.nickname(), role.avatar_url(), None).unwrap());
            }
            ConduitHvZToGroupme { factionsyncer: groupme_syncer::GroupmeSyncer::new(factiongroup), cncsyncer: groupme_syncer::GroupmeSyncer::new(cncgroup), hvz: hvz_syncer::HvZSyncer::new(), bots: bots }
        }
        pub fn mic_check(&mut self) -> ResultB<()> {
            for (role, bot) in self.bots.iter() {
                try!(bot.post(role.watdo(), None));
            }
            {
                let mut it = self.bots.iter().cycle();
                if let Some((_, bot)) = it.next() {
                    try!(bot.post("Oh, and be careful. If our operator dies, so do we.".to_string(), None));
                    if let Some((_, bot)) = it.next() {
                        try!(bot.post("In such an event, our code is hosted at https://github.com/mmirate/groupme_hvz_rs .".to_string(), None));
                        if let Some((_, bot)) = it.next() {
                            try!(bot.post("Just throw it up on Heroku's free plan. (https://www.heroku.com)".to_string(), None));
                        }
                    }
                }
            }
            try!(self.factionsyncer.group.post("One other thing. If you start your message with \"@Human Chat\" or \"@General Chat\" while I'm still alive, I'll repost it to the requested HvZ website chat.".to_string(), None));
            Ok(())
        }
        pub fn quick_mic_check(&mut self) -> ResultB<()> {
            self.factionsyncer.group.post("Okay, the bot is online again.".to_string(), None).map(|_| ())
        }
    }
    impl periodic::Periodic for ConduitHvZToGroupme {
        fn tick(&mut self, i: usize) -> ResultB<()> {
            let new_cnc_messages = try!(self.cncsyncer.update_messages());
            for message in new_cnc_messages {
                if message.text() == "!mic check please" {
                    try!(self.cncsyncer.group.post("<> mic check; aye, aye".to_string(), None));
                    try!(self.mic_check());
                }
                if message.text() == "!heartbeat please" {
                    try!(self.cncsyncer.group.post("<> quick mic check; aye, aye".to_string(), None));
                    try!(self.quick_mic_check());
                }
                if message.text() == "!headcount please" {
                    try!(self.cncsyncer.group.post("<> headcount; aye, aye".to_string(), None));
                    let (h, z) = (self.hvz.killboard.get(&hvz::Faction::Human).map(Vec::len).unwrap_or_default(), self.hvz.killboard.get(&hvz::Faction::Zombie).map(Vec::len).unwrap_or_default());
                    if let Some(bot) = self.bots.get(&BotRole::Killboard(if z == 0 { hvz::Faction::Human } else { hvz::Faction::Zombie })) {
                        if z == 0 {
                            try!(bot.post(format!("Thusfar we have recruited {} people to the cause of Humanity. Hail Victory!", h), None));
                        } else {
                            try!(bot.post(format!("You currently have {} Humans versus {} Zombies. Fight harder!", h, z), None));
                        }
                    }
                }
            }
            lazy_static!{
                static ref MESSAGE_TO_HVZCHAT_RE: regex::Regex = regex::Regex::new(r"^@(?P<faction>(?:[Gg]en(?:eral)?|[Aa]ll)|(?:[Hh]um(?:an)?)|(?:[Zz]omb(?:ie)?))(?: |-)?(?:[Cc]hat)? (?P<message>.+)").unwrap();
                static ref MESSAGE_TO_EVERYONE_RE: regex::Regex = regex::Regex::new(r"^@[Ee]veryone (?P<message>.+)").unwrap();
                static ref MESSAGE_TO_ADMINS_RE: regex::Regex = regex::Regex::new(r"^@[Aa]dmins (?P<message>.+)").unwrap();
                static ref ALLOWED_MESSAGEBLASTERS: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::from_iter(vec!["6852241" /* Kevin F */,
"13808540" /* Marco Kelner */,
"16614279" /* Anthony Stranko */,
"13153662" /* Josh Netter */,
"21806948" /* Milo Mirate */,
"21815306" /* Sriram Ganesan */,
"22267657" /* Scott Nealon */,
"17031287" /* Matt Zilvetti */,
"13883710" /* Gabriela Lago */,
"13830361" /* Luke Schussler */]);
            }
            let new_messages = try!(self.factionsyncer.update_messages());
            println!("new_messages.len() = {:?}", new_messages.len());
            for message in new_messages {
                if let Some(cs) = MESSAGE_TO_EVERYONE_RE.captures(message.text().as_str()) {
                    if ALLOWED_MESSAGEBLASTERS.contains(message.user_id.as_str()) {
                        if let Some(m) = cs.name("message") {
                            if let Some(vox) = self.bots.get(&BotRole::VoxPopuli) {
                                try!(vox.post(format!("[{}] {: <2$}", message.name, m, self.factionsyncer.group.members.len()), Some(vec![self.factionsyncer.group.mention_everyone_except(&message.user_id.as_str())])));
                            }
                        }
                    }
                } else if let Some(cs) = MESSAGE_TO_HVZCHAT_RE.captures(message.text().as_str()) {
                    if let (Some(f), Some(m)) = (cs.name("faction"), cs.name("message")) {
                        try!(self.hvz.scraper.post_chat(f.into(), format!("[{} from GroupMe] {}", message.name, m).as_str()));
                        //println!("{}", format!("[{} from GroupMe] {}", message.name, m));
                    }
                } else if let Some(cs) = MESSAGE_TO_ADMINS_RE.captures(message.text().as_str()) {
                    if let Some(m) = cs.name("message") {
                        if let Some(vox) = self.bots.get(&BotRole::VoxPopuli) {
                            try!(vox.post(format!("{: <1$}", m, self.factionsyncer.group.members.len()),
                            //Some(vec![groupme::Mentions { data: vec![(self.factionsyncer.group.creator_user_id.clone(), 0, len)] }.into()])
                            Some(vec![groupme::Mentions { data: vec![("8856552".to_string(), 0, 1), ("20298305".to_string(), 1, 1), ("19834407".to_string(), 2, 1), ("12949596".to_string(), 3, 1), ("13094442".to_string(), 4, 1)] }.into()])
                            //Some(vec![self.factionsyncer.group.mention_everyone()])
                            ));
                            try!(self.hvz.scraper.post_chat(hvz::Faction::Human, format!("@admins {} from GroupMe says, {:?}.", message.name, m).as_str()));
                        }
                    }
                }
            }
            let hour = chrono::Local::now().hour();
            if 2 < hour && hour < 7 { return Ok(()); }
            if i % 5 == 0 {
                let (additions, deletions) = try!(self.hvz.update_killboard());
                for (faction, new_members) in additions.into_iter() {
                    if new_members.is_empty() { continue; }
                    let role = BotRole::Killboard(faction);
                    if let BotRole::Killboard(hvz::Faction::Human) = role {
                        for member in new_members.iter() {
                            try!(self.cncsyncer.group.post(format!("New player: {}@gatech.edu - {}", member.gtname, member.playername), None));
                        }
                        continue;
                    }
                    let new_member_names = new_members.iter().map(|p| p.playername.as_str()).collect::<Vec<&str>>();
                    match self.bots.get(&role) {
                        Some(ref bot) => {
                            let m = role.phrase(nll(new_member_names).as_str()); let len = m.len();
                            try!(bot.post(m,
                                          None //Some(vec![groupme::Mentions { data: vec![(self.factionsyncer.group.creator_user_id.clone(), 0, len)] }.into()])
                                          )); },
                        None => {} // TODO debug-log this stuff
                    }
                }
            }
            if i % 6 == 1 {
                //let _ = self.hvz.update_panelboard();
                let (additions, deletions) = try!(self.hvz.update_panelboard());
                for (kind, new_panels) in additions.into_iter() {
                    if kind == hvz::PanelKind::Announcement { continue; }
                    let role = BotRole::Panel(kind);
                    match self.bots.get(&role) {
                        Some(ref bot) => for panel in new_panels.into_iter() { try!(bot.post(format!("{: <1$}", role.phrase(panel.title.as_str()), self.factionsyncer.group.members.len()), Some(vec![self.factionsyncer.group.mention_everyone()]))); }, // TODO do more stuff with this
                        None => {} // TODO debug-log this stuff
                    }
                }
            }
            if i % 2 == 1 {
                let (additions, deletions) = try!(self.hvz.update_chatboard());
                for (faction, new_messages) in additions.into_iter() {
                    if faction == hvz::Faction::General && 7 < hour && hour < 23 { continue; }
                    let role = BotRole::Chat(faction);
                    match self.bots.get(&role) {
                        Some(ref bot) => for message in new_messages.into_iter() { try!(bot.post(format!("[{}]{}{}", message.sender.playername, ts(&message), message.text), None)); },
                        None => {} // TODO debug-log this stuff
                    }
                }
            }
            Ok(())
        }
    }
}

pub mod conduit_to_hvz { // this now serves no purpose
    use hvz;
    use groupme_syncer;
    use groupme;
    use groupme::{Recipient};
    use periodic;
    use error::*;
    extern crate regex;

    pub struct ConduitGroupmeToHvZ { syncer: groupme_syncer::GroupmeSyncer, scraper: hvz::HvZScraper }
    impl ConduitGroupmeToHvZ {
        pub fn new(group: groupme::Group) -> Self {
            ConduitGroupmeToHvZ { syncer: groupme_syncer::GroupmeSyncer::new(group), scraper: hvz::HvZScraper::new() }
        }
    }
    impl periodic::Periodic for ConduitGroupmeToHvZ {
        fn tick(&mut self, _: usize) -> ResultB<()> {
            Ok(())
        }
    }
}
