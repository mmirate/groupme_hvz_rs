#![recursion_limit = "2048"]
#![deny(warnings)]
extern crate chrono;
extern crate hyper;
extern crate multipart;
extern crate openssl;
extern crate postgres;
extern crate rand;
extern crate regex;
extern crate rustc_serialize;
extern crate rusttype;
extern crate scraper;
extern crate time;
extern crate url;
extern crate users;
extern crate image;
#[macro_use(static_slice)] extern crate static_slice;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate error_chain;
pub mod groupme;
pub mod hvz;
pub mod syncer;
pub mod render;

pub mod errors {
    error_chain! {
        foreign_links {
            Hyper (::hyper::Error);
            Io (::std::io::Error);
            Postgres (::postgres::error::Error);
            JsonEncoding (::rustc_serialize::json::EncoderError);
            JsonDecoding (::rustc_serialize::json::DecoderError);
            JsonParsing (::rustc_serialize::json::ParserError);
            UrlParsing (::url::ParseError);
            DateFormatParsing (::chrono::format::ParseError);
        }
        errors {
            RLE {}
            HttpError(s: ::hyper::status::StatusCode) {
                from()
                description("HTTP request returned error.")
                display("HTTP error: {:?}.", s)
            }
            TextTooLong(text: String, attachments: Option<Vec<::rustc_serialize::json::Json>>) {
                description("A message was too long for GroupMe.")
                display("Message {:?} cannot be sent via GroupMe.", text)
            }
            CSS {
                description("A CSS selector failed to parse.")
                display("A CSS selector failed to parse.")
            }
            Scraper(s: &'static str) {
                description("Scraping failed.")
                display("Scraping {} failed.", s)
            }
            GaTechCreds {
                description("Login credentials are wrong.")
            }
            BandwidthLimitExceeded { // code 509
                description("Georgia Tech website bandwidth limit exceeded.")
            }
            AteData(role: ::conduit_to_groupme::BotRole) {
                description("A bot could not be found.")
                display("Bot {:?} not found.", role)
            }
        }
    }
}
            //BadPixelFormat {}
            //BadBufferSize {}
//    use futures::Future;
//    use futures::stream::Stream;
//    pub type Future<T> = Future<Item=T, Error=Error>;
//    pub type Stream<T> = Stream<Item=T, Error=Error>;

//use errors::*;

pub mod hvz_syncer {
    use std;
    use hvz;
    use syncer;
    use postgres;
    use errors::*;
    #[derive(Debug)] pub struct HvZSyncer { pub killboard: hvz::Killboard, pub chatboard: hvz::Chatboard, pub panelboard: hvz::Panelboard, pub scraper: hvz::HvZScraper, conn: postgres::Connection, }
    pub type Changes<T> = (T, T);
    impl HvZSyncer {
        pub fn new(username: String, password: String) -> HvZSyncer {
            let mut me = HvZSyncer { scraper: hvz::HvZScraper::new(username, password), conn: syncer::setup(), killboard: hvz::Killboard::new(), chatboard: hvz::Chatboard::new(), panelboard: hvz::Panelboard::new(), };
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
        pub fn update_killboard(&mut self) -> Result<Changes<hvz::Killboard>> {
            let (killboard, additions, deletions) = try!(syncer::update_map(&self.conn, "killboard", try!(self.scraper.fetch_killboard()), &mut self.killboard, true));
            self.killboard = killboard;
            Ok((additions, deletions))
        }
        #[inline] pub fn new_zombies(&mut self) -> Result<Vec<hvz::Player>> { Ok(try!(self.update_killboard()).0.remove(&hvz::Faction::Human).unwrap_or(vec![])) }
        pub fn update_chatboard(&mut self) -> Result<Changes<hvz::Chatboard>> {
            let (chatboard, additions, deletions) = try!(syncer::update_map(&self.conn, "chatboard", try!(self.scraper.fetch_chatboard()), &mut self.chatboard, true));
            self.chatboard = chatboard;
            Ok((additions, deletions))
        }
        pub fn update_panelboard(&mut self) -> Result<Changes<hvz::Panelboard>> {
            let (panelboard, additions, deletions) = try!(syncer::update_map(&self.conn, "panelboard", try!(self.scraper.fetch_panelboard()), &mut self.panelboard, false));
            self.panelboard = panelboard;
            Ok((additions, deletions))
        }
    }
}

pub mod groupme_syncer {
    use syncer;
    use postgres;
    use groupme;
    use groupme::BidirRecipient;
    use errors::*;

    #[derive(Debug)] pub struct GroupmeSyncer { pub group: groupme::Group, pub last_message_id: Option<String>, pub members: Vec<groupme::Member>, conn: postgres::Connection, }
    impl GroupmeSyncer {
        pub fn new(group: groupme::Group) -> GroupmeSyncer {
            let conn = syncer::setup();
            let last_message_id = syncer::read(&conn, (group.group_id.clone() + "last_message_id").as_str()).ok();
            GroupmeSyncer { group: group, last_message_id: last_message_id, members: vec![], conn: conn }
        }
        pub fn update_messages(&mut self) -> Result<Vec<groupme::Message>> {
            let last_message_id = self.last_message_id.clone();
            println!("last_message_id = {:?}", &last_message_id);
            let selector = last_message_id.clone().map(groupme::MessageSelector::After);
            //let selector = match last_message_id {
            //    Some(ref m) => Some(groupme::MessageSelector::After(m.clone())),
            //    None => None,
            //};
            println!("selector = {:?}", selector);
            let ret = try!(self.group.slurp_messages(selector.clone()));
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
    use errors::*;
    pub trait Periodic {
        fn tick(&mut self, usize) -> Result<()>;
    }
}

pub mod conduit_to_groupme { // A "god" object. What could go wrong?
    use std;                 // *500 LOC later*: that.
    use hvz::{self, KillboardExt};
    use hvz_syncer;
    use groupme_syncer;
    extern crate chrono;
    use self::chrono::Timelike;
    use groupme;
    use groupme::{Recipient};
    use periodic;
    use rustc_serialize;
    use rand;
    use rand::Rng;
    //use rustc_serialize::json::ToJson;
    use std::convert::Into;
    use std::iter::FromIterator;
    use errors::*;
    use regex;

    fn nll(items: Vec<&str>, postlude: Option<&str>) -> String {
        if let Some((tail, init)) = items.split_last() {
            if let Some((_, _)) = init.split_last() {
                format!("{} and {}{}", init.join(", "), tail.to_string(), postlude.unwrap_or(""))
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

    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] pub enum BotRole { VoxPopuli, Chat(hvz::Faction), Killboard(hvz::Faction), Panel(hvz::PanelKind) }
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
                &BotRole::VoxPopuli => "If certain people start your message with \"@Everyone\" or \"@everyone\", I'll repost it in such a way that it will mention everyone. Abuse this, and there will be consequences.".to_string(),
                &BotRole::Chat(f) => format!("I'm the voice of {} chat. When someone posts something there, I'll tell you about it within a few seconds.{}", f, if f == hvz::Faction::General { " Except during gameplay hours, because spam is bad." } else { "" }),
                &BotRole::Killboard(hvz::Faction::Zombie) => "If someone shows up on the other side of the killboard, I'll report it here within about a minute, and simultaneously try to go about kicking them. If I can't, I'll give a holler.".to_string(),
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
            let _f: fn(&str) -> String = {
                fn _f(x: &str) -> String { format!("{:?}", x) }
                _f
            };
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
            (*rand::thread_rng().choose(formats).unwrap_or(&_f))(param)
        }
        fn phrase_2(&self, param1: &str, param2: &str) -> String {
            macro_rules! messages {
                ($($fmtstr:tt),*) => { static_slice![fn(&str, &str) -> String: $({
                        fn f(x: &str, y: &str) -> String { format!($fmtstr, x, y) }
                        f
                }),*] }
            }
            let _f: fn(&str, &str) -> String = {
                fn _f(x: &str, y: &str) -> String { format!("{:?}{:?}", x, y) }
                _f
            };
            let formats: &[fn(&str, &str) -> String] = match self {
                &BotRole::VoxPopuli => messages!["{}{}"],
                &BotRole::Chat(hvz::Faction::Zombie) => messages!["{}{}"],
                &BotRole::Chat(hvz::Faction::Human) => messages!["{}{}"],
                &BotRole::Chat(_) => messages!["{:?}{:?}"],
                &BotRole::Killboard(hvz::Faction::Zombie) => messages!["Verdammt! We lost {} to {}. I hope it was worth it.", "(－‸ლ) {} died (to {}). Come on; we can do better than this!", "Well, I'll be. Looks like {} bit the dust of {}.", "Well, drat. I think the zombies got {}. (\"the zombies\" being {}) I hope they died with dignity.", "Well, I declare. Seems that {} kicked the bucket, kudos to {}.", "Hunh. I guess {} turned (due to {}). A grim inevitability.", "Are you kidding me? Killboard says they've nommed {}! (Or rather, {} nommed them.) Fight harder!", "Argh. {} passed on to the undeath, with the help of {}."],
                &BotRole::Killboard(hvz::Faction::Human) => messages!["({:?}, {:?})"],
                &BotRole::Killboard(_) => messages!["{:?}{:?}"],
                &BotRole::Panel(hvz::PanelKind::Mission) => messages!["{:?}{:?}"],
                &BotRole::Panel(hvz::PanelKind::Announcement) => messages!["{:?}{:?}"],
            };
            (*rand::thread_rng().choose(formats).unwrap_or(&_f))(param1, param2)
        }
    }

    fn ts(m: &hvz::Message) -> String {
        let min = (chrono::Local::now() - m.timestamp).num_minutes();
        if min > 60 { m.timestamp.format(" [%a %H:%M:%S] ").to_string() }
        else if min > 2 { m.timestamp.format(" [%H:%M:%S] ").to_string() }
        else { " ".to_string() }
    }

    #[derive(RustcDecodable, RustcEncodable)] struct RuntimeState { missions: bool, annxs: bool, dormant: bool, throttled_at: i64 }
    impl RuntimeState {
        fn write(&self, cncgroup: &mut groupme::Group) -> Result<()> {
            cncgroup.update(None, try!(rustc_serialize::json::encode(&self)).into(), None, None).map(|_| ())
        }
    }
    impl Default for RuntimeState { fn default() -> Self { RuntimeState { missions: false, annxs: false, dormant: true, throttled_at: 0i64 } } }

    pub struct ConduitHvZToGroupme { factionsyncer: groupme_syncer::GroupmeSyncer, cncsyncer: groupme_syncer::GroupmeSyncer, hvz: hvz_syncer::HvZSyncer, bots: std::collections::BTreeMap<BotRole, groupme::Bot>, state: RuntimeState }
    impl ConduitHvZToGroupme {
        pub fn new(factiongroup: groupme::Group, mut cncgroup: groupme::Group, username: String, password: String) -> Self {
            let mut bots = std::collections::BTreeMap::new();
            for role in vec![BotRole::VoxPopuli, BotRole::Chat(hvz::Faction::Human), BotRole::Chat(hvz::Faction::General), BotRole::Killboard(hvz::Faction::Human), BotRole::Killboard(hvz::Faction::Zombie), BotRole::Panel(hvz::PanelKind::Mission), BotRole::Panel(hvz::PanelKind::Announcement)].into_iter() {
                bots.insert(role, groupme::Bot::upsert(&factiongroup, role.nickname(), role.avatar_url(), None).unwrap());
            }
            let tutorial = cncgroup.description.is_none();
            let state = match rustc_serialize::json::decode::<RuntimeState>(cncgroup.description.as_ref().map(AsRef::as_ref).unwrap_or("")) {
                Ok(s) => s,
                _ => { let s = RuntimeState::default(); s.write(&mut cncgroup).unwrap(); s }
            };
            cncgroup.post(format!("<> bot starting up; in {} state. please say \"!wakeup\" to exit the dormant state, or \"!sleep\" to enter it.", if state.dormant { "DORMANT" } else { "ACTIVE" }), None).unwrap();
            if tutorial {
                cncgroup.post("<> annunciation of new missions and announcements can be toggled at any time. To do so, say \"!missions on\", \"!missions off\", \"!annx on\" or \"!annx off\".".to_owned(), None).unwrap();
            }
            ConduitHvZToGroupme { factionsyncer: groupme_syncer::GroupmeSyncer::new(factiongroup), cncsyncer: groupme_syncer::GroupmeSyncer::new(cncgroup), hvz: hvz_syncer::HvZSyncer::new(username, password), bots: bots, state: state }
        }
        pub fn mic_check(&mut self) -> Result<()> {
            for (role, bot) in self.bots.iter() {
                try!(bot.post(role.watdo(), None));
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
            {
                let mut it = self.bots.iter().cycle();
                if let Some((_, bot)) = it.next() {
                    try!(bot.post("Oh, and be careful. If our operator dies, so do we.".to_string(), None));
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    if let Some((_, bot)) = it.next() {
                        try!(bot.post("In such an event, our code is hosted at https://github.com/mmirate/groupme_hvz_rs .".to_string(), None));
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        if let Some((_, bot)) = it.next() {
                            try!(bot.post("Just throw it up on Heroku's free plan. (https://www.heroku.com)".to_string(), None));
                            std::thread::sleep(std::time::Duration::from_secs(3));
                        }
                    }
                }
            }
            try!(self.factionsyncer.group.post("Two more things. (1) If you start your message with \"@Human Chat\" or \"@General Chat\", I'll repost it to the requested HvZ website chat.".to_string(), None));
            std::thread::sleep(std::time::Duration::from_secs(3));
            try!(self.factionsyncer.group.post("(2) If your message includes the two words \"I'm dead\" adjacently and in that order, but without the doublequotes and regardless of capitalization or non-doublequote punctuation ... I will kick you from the Group within a few seconds.".to_string(), None));
            std::thread::sleep(std::time::Duration::from_secs(3));
            Ok(())
        }
        pub fn quick_mic_check(&mut self) -> Result<()> {
            let text = if self.state.dormant { "Okay, backup bot is up. Ping me if the current bot-operator dies." } else { "Okay, the bot is online again." };
            self.factionsyncer.group.post(text.to_string(), None).map(|_| ())
        }

        fn process_cnc(&mut self, _i: usize) -> Result<()> {
            let new_cnc_messages = try!(self.cncsyncer.update_messages());
            for message in new_cnc_messages {
                if !message.favorited_by.is_empty() { continue; }
                let _text = message.text().clone();
                let words = _text.split_whitespace().collect::<Vec<_>>();
                if words == ["!mic", "check", "please"] {
                    try!(self.cncsyncer.group.post("<> mic check; aye, aye".to_string(), None));
                    try!(message.like());
                    try!(self.mic_check());
                }
                if words == ["!heartbeat", "please"] {
                    try!(self.cncsyncer.group.post("<> quick mic check; aye, aye".to_string(), None));
                    try!(message.like());
                    try!(self.quick_mic_check());
                }
                if words == ["!headcount", "please"] {
                    try!(self.cncsyncer.group.post("<> headcount; aye, aye".to_string(), None));
                    try!(message.like());
                    let (h, z) = (self.hvz.killboard.get(&hvz::Faction::Human).map(Vec::len).unwrap_or_default(), self.hvz.killboard.get(&hvz::Faction::Zombie).map(Vec::len).unwrap_or_default());
                    if let Some(bot) = self.bots.get(&BotRole::Killboard(if z == 0 { hvz::Faction::Human } else { hvz::Faction::Zombie })) {
                        if z == 0 {
                            try!(bot.post(format!("Thusfar we have recruited {} people to the cause of Humanity. Hail Victory!", h), None));
                        } else {
                            try!(bot.post(format!("You currently have {} Humans versus {} Zombies. Fight harder!", h, z), None));
                        }
                    }
                }
                if words.get(0) == Some(&"!missions") {
                    match words.get(1) {
                        Some(&"on") => {
                            try!(self.cncsyncer.group.post("<> mission annunciation ON; aye, aye".to_string(), None));
                            try!(message.like());
                            self.state.missions = true;
                            try!(self.state.write(&mut self.cncsyncer.group));
                        },
                        Some(&"off") => {
                            try!(self.cncsyncer.group.post("<> mission annunciation OFF; aye, aye".to_string(), None));
                            try!(message.like());
                            self.state.missions = false;
                            try!(self.state.write(&mut self.cncsyncer.group));
                        },
                        _ => {},
                    }
                }
                if words.get(0).map(|s| s.starts_with("!ann")).unwrap_or(false) {
                    match words.get(1) {
                        Some(&"on") => {
                            try!(self.cncsyncer.group.post("<> announcement annunciation ON; aye, aye".to_string(), None));
                            try!(message.like());
                            self.state.annxs = true;
                            try!(self.state.write(&mut self.cncsyncer.group));
                        },
                        Some(&"off") => {
                            try!(self.cncsyncer.group.post("<> announcement annunciation OFF; aye, aye".to_string(), None));
                            try!(message.like());
                            self.state.annxs = false;
                            try!(self.state.write(&mut self.cncsyncer.group));
                        },
                        _ => {},
                    }
                }
                if words.get(0) == Some(&"!wakeup") {
                    try!(self.cncsyncer.group.post("<> waking up; aye, aye. good luck, operator. say \"!heartbeat please\" if you wish to announce my awakening.".to_string(), None));
                    try!(self.cncsyncer.group.update(None, "active".to_string().into(), None, None));
                    try!(message.like());
                    self.state.dormant = false;
                    try!(self.state.write(&mut self.cncsyncer.group));
                }
                if words.get(0) == Some(&"!sleep") {
                    try!(self.cncsyncer.group.post("<> going to sleep; aye, aye".to_string(), None));
                    try!(self.cncsyncer.group.update(None, "dormant".to_string().into(), None, None));
                    try!(message.like());
                    self.state.dormant = true;
                    try!(self.state.write(&mut self.cncsyncer.group));
                }
                if words.get(0) == Some(&"!dead") {
                    try!(self.cncsyncer.group.post("<> going to sleep; aye, aye. may the horde be with you.".to_string(), None));
                    try!(self.cncsyncer.group.update(None, "dormant".to_string().into(), None, None));
                    try!(message.like());
                    self.state.dormant = true;
                    try!(self.state.write(&mut self.cncsyncer.group));
                }
            }
            Ok(())
        }

        fn process_groupme_messages(&mut self, _i: usize) -> Result<()> {
            lazy_static!{
                static ref MESSAGE_TO_HVZCHAT_RE: regex::Regex = regex::Regex::new(r"^@(?P<faction>(?:[Gg]en(?:eral)?|[Aa]ll)|(?:[Hh]um(?:an)?)|(?:[Zz]omb(?:ie)?))(?: |-)?(?:[Cc]hat)? (?P<message>.+)").unwrap();
                static ref MESSAGE_TO_EVERYONE_RE: regex::Regex = regex::Regex::new(r"^@[Ee]veryone (?P<message>.+)").unwrap();
                static ref MESSAGE_TO_ADMINS_RE: regex::Regex = regex::Regex::new(r"^@[Aa]dmins (?P<message>.+)").unwrap();
                static ref ALLOWED_MESSAGEBLASTERS: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::from_iter(vec![
"16614279" /* Anthony Stranko */,
"11791190" /* Cameron Braun */,
"13883710" /* Gabriela Lago */,
"13153662" /* Josh Netter */,
"6852241" /* Kevin F */,
"13830361" /* Luke Schussler */,
"13808540" /* Marco Kelner */,
"17031287" /* Matt Zilvetti */,
"21806948" /* Milo Mirate */,
"22267657" /* Scott Nealon */,
"21815306" /* Sriram Ganesan */,
                ]);
            }
            let (hour, _minute) = { let n = chrono::Local::now(); (n.hour(), n.minute()) };
            let new_messages = try!(self.factionsyncer.update_messages());
            println!("new_messages.len() = {:?}", new_messages.len());
            let me = try!(groupme::User::get());
            for message in new_messages {
                if 7 <= hour && hour < 23 {
                    let signature = format!(" {} ", message.text().to_lowercase().split_whitespace().map(|word| word.replace(|c: char| { !c.is_alphabetic() && c != '"' && c != '?' }, "")).collect::<Vec<String>>().join(" "));
                    if signature.contains(" im dead ") || signature.contains(" i am dead ") {
                        if !([&self.factionsyncer.group.creator_user_id, &me.user_id].contains(&&message.user_id)) {
                            if let Some(member) = self.factionsyncer.group.members.iter().find(|&m| m.user_id == message.user_id) {
                                if let Err(_) = self.factionsyncer.group.remove(member.clone()) {
                                    //try!(self.factionsyncer.group.post("... I guess they already got kicked?".to_owned(), None).map(|_| ())) // Actually, we don't care about this.
                                }
                            }
                            try!(self.factionsyncer.group.refresh());
                        }
                    }
                }
                if self.state.dormant { continue; }
                if let Some(cs) = MESSAGE_TO_EVERYONE_RE.captures(message.text().as_str()) {
                    if ALLOWED_MESSAGEBLASTERS.contains(message.user_id.as_str()) {
                        if let Some(m) = cs.name("message") {
                            if let Some(vox) = self.bots.get(&BotRole::VoxPopuli) {
                                try!(vox.post(format!("[{}] {: <2$}", message.name, m.as_str(), self.factionsyncer.group.members.len()), Some(vec![self.factionsyncer.group.mention_everyone_except(&message.user_id.as_str())])));
                            }
                        }
                    }
                } else if let Some(cs) = MESSAGE_TO_HVZCHAT_RE.captures(message.text().as_str()) {
                    if let (Some(f), Some(m)) = (cs.name("faction"), cs.name("message")) {
                        try!(self.hvz.scraper.post_chat(f.as_str().into(), format!("[{} from GroupMe] {}", message.name, m.as_str()).as_str()));
                    }
                } else if let Some(cs) = MESSAGE_TO_ADMINS_RE.captures(message.text().as_str()) { // TODO REDO
                    if let Some(m) = cs.name("message") {
                        if let Some(vox) = self.bots.get(&BotRole::VoxPopuli) {
                            try!(vox.post(format!("{: <1$}", m.as_str(), self.factionsyncer.group.members.len()),
                            //Some(vec![groupme::Mentions { data: vec![(self.factionsyncer.group.creator_user_id.clone(), 0, len)] }.into()])
                            Some(vec![groupme::Mentions { data: vec![("8856552".to_string(), 0, 1), ("20298305".to_string(), 1, 1), ("19834407".to_string(), 2, 1), ("12949596".to_string(), 3, 1), ("13094442".to_string(), 4, 1)] }.into()])
                            //Some(vec![self.factionsyncer.group.mention_everyone()])
                            ));
                            //try!(self.hvz.scraper.post_chat(hvz::Faction::Human, format!("@admins {} from GroupMe says, {:?}.", message.name, m).as_str()));
                        }
                    }
                }
            }
            Ok(())
        }

        fn process_killboard(&mut self, i: usize) -> Result<()> {
            let (hour, _minute) = { let n = chrono::Local::now(); (n.hour(), n.minute()) };
            if 2 < hour && hour < 7 { return Ok(()); }
            if i % 8 == 0 {
                let (additions, _deletions) = try!(self.hvz.update_killboard());
                for (faction, new_members) in additions.into_iter() {
                    if new_members.is_empty() { continue; }
                    let role = BotRole::Killboard(faction);
                    if let BotRole::Killboard(hvz::Faction::Human) = role {
                        for member in new_members.iter() {
                            try!(self.cncsyncer.group.post(format!("<> new player: {}@gatech.edu - {}", member.gtname, member.playername), None));
                        }
                        continue;
                    }
                    if self.state.dormant { continue; }
                    match self.bots.get(&role) {
                        Some(ref bot) => {
                            let new_member_names = new_members.iter().map(|p| p.playername.as_str()).collect::<Vec<&str>>();
                            let m = match new_members.iter().map(|p| p.kb_playername.clone()).collect::<Option<Vec<String>>>() {
                                Some(perpetrators) => role.phrase_2(nll(new_member_names, None).as_str(), nll(perpetrators.iter().map(AsRef::as_ref).collect(), ", resp.".into()).as_str()),
                                None => role.phrase(nll(new_member_names, None).as_str())
                            };
                            //let len = m.len();
                            try!(bot.post(m,
                                        None //Some(vec![groupme::Mentions { data: vec![(self.factionsyncer.group.creator_user_id.clone(), 0, len)] }.into()])
                                        )); },
                        None => { bail!(ErrorKind::AteData(role)) }
                    }
                    if faction == hvz::Faction::Zombie { // redundant conditional; keep it in case it becomes non-redundant
                        try!(self.factionsyncer.group.refresh());
                        let (mut removals, mut removalfailures) = (vec![], vec![]);
                        'player: for zombie_player in new_members {
                            for member in self.factionsyncer.group.members.iter() {
                                if member.canonical_name().to_lowercase() == zombie_player.playername.to_lowercase() {
                                    removals.push((zombie_player, member.to_owned()));
                                    continue 'player;
                                }
                            }
                            if self.hvz.killboard.name_has_ambiguous_surname(&zombie_player.playername) {
                                removalfailures.push(zombie_player);
                                continue 'player;
                            }
                            for member in self.factionsyncer.group.members.iter() {
                                if hvz::Killboard::surname(&member.canonical_name()).to_lowercase() == hvz::Killboard::surname(&zombie_player.playername).to_lowercase() {
                                    removals.push((zombie_player, member.to_owned()));
                                    continue 'player;
                                }
                            }
                            removalfailures.push(zombie_player);
                            continue 'player;
                        }
                        for (death, removal) in removals.into_iter() {
                            match self.factionsyncer.group.remove_mut(removal) {
                                Ok(_) => {},
                                Err(_) => { removalfailures.push(death); }
                            }
                        }
                        if !removalfailures.is_empty() {
                            match self.bots.get(&role) {
                                Some(ref bot) => {
                                    let names = removalfailures.into_iter().map(|p| p.playername).collect::<Vec<_>>();
                                    try!(bot.post(format!("DANGER! Of those deaths, I failed to kick {}.", nll(names.iter().map(|ref s| s.as_str()).collect::<Vec<_>>(), None)), None));
                                },
                                None => { bail!(ErrorKind::AteData(role)) }
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        fn process_panelboard(&mut self, i: usize) -> Result<()> {
            let (hour, minute) = { let n = chrono::Local::now(); (n.hour(), n.minute()) };
            if 2 < hour && hour < 7 { return Ok(()); }
            if (15 - ((minute as i32)%30)).abs() >= 12 && i % 4 == 2 /*i % 6 == 1*/ {
                let (additions, _deletions) = try!(self.hvz.update_panelboard());
                for (kind, new_panels) in additions.into_iter() {
                    if kind == hvz::PanelKind::Announcement && !self.state.annxs { continue; }
                    if kind == hvz::PanelKind::Mission && !self.state.missions { continue; }
                    if self.state.dormant { continue; }
                    if new_panels.is_empty() { continue; }
                    let role = BotRole::Panel(kind);
                    match self.bots.get(&role) {
                        Some(ref bot) => for panel in new_panels.into_iter() {
                            if kind == hvz::PanelKind::Mission && panel.particulars.map(|p| (p.start - chrono::Local::now()).num_minutes()).map(|m| m > 65 || m < 15).unwrap_or(true) { continue; } // only post about missions that are actually upcoming
                            try!(bot.post_or_post_image(format!("{: <2$}\n{}", role.phrase(panel.title.as_str()), panel.text, self.factionsyncer.group.members.len()), Some(vec![self.factionsyncer.group.mention_everyone()])));
                        },
                        None => { bail!(ErrorKind::AteData(role)) }
                    }
                }
            }
            Ok(())
        }

        fn process_chatboard(&mut self, i: usize) -> Result<()> {
            let (hour, _minute) = { let n = chrono::Local::now(); (n.hour(), n.minute()) };
            if 2 < hour && hour < 7 { return Ok(()); }
            if i % 8 == 1 || (!self.state.dormant && i % 4 == 1) {
                let (additions, _deletions) = try!(self.hvz.update_chatboard());
                for (faction, new_messages) in additions.into_iter() {
                    if new_messages.is_empty() { continue; }
                    if faction == hvz::Faction::General && 7 <= hour && hour < 23 { continue; }
                    if self.state.dormant { continue; }
                    let role = BotRole::Chat(faction);
                    match self.bots.get(&role) {
                        Some(ref bot) => for message in new_messages.into_iter() { try!(bot.post(format!("[{}]{}{}", message.sender.playername, ts(&message), message.text), None)); },
                        None => { bail!(ErrorKind::AteData(role)) }
                    }
                }
            }
            Ok(())
        }

        fn being_throttled(&mut self, e: Result<()>) -> Result<()> {
            let now = chrono::Local::now().timestamp();
            if now - self.state.throttled_at < 60*15 { e } else {
                self.state.throttled_at = now;
                let mut it = self.bots.iter();
                if let Some((_, bot)) = it.next() {
                    if self.state.dormant { e } else {
                        let mut ret = vec![];
                        ret.push(e);
                        ret.push(bot.post("Ouch. We're being throttled, so we have to go down for 15 minutes. Please check the killboard while we're gone!".to_owned(), None).map(|_| ())); // TODO ping someone here
                        ret.into_iter().collect::<Result<Vec<()>>>().map(|_: Vec<_>| ())
                    }
                } else { println!("HOLY S*** I JUST ATE SOME DATA!"); e }
            }
        }

    }
    impl periodic::Periodic for ConduitHvZToGroupme {
        fn tick(&mut self, i: usize) -> Result<()> {
            // non-use of try!() is intentional here; we want to attempt each function even if one fails
            let mut ret = vec![];
            ret.push(self.process_cnc(i));
            ret.push(self.process_groupme_messages(i));
            let now = chrono::Local::now().timestamp();
            if now - self.state.throttled_at > 60*15 {
                println!("Not feeling throttled atm.");
                ret.push(self.process_killboard(i));
                ret.push(self.process_panelboard(i));
                ret.push(self.process_chatboard(i));
            }
            match ret.into_iter().collect::<Result<Vec<()>>>().map(|_: Vec<_>| ()) {
                x @ Err(Error(ErrorKind::BandwidthLimitExceeded, _)) => {
                    self.being_throttled(x)
                },
                x => x
            }
        }
    }
}

//pub mod conduit_to_hvz { // this now serves no purpose
//    use hvz;
//    use groupme_syncer;
//    use groupme;
//    use groupme::{Recipient};
//    use periodic;
//    use errors::*;
//    extern crate regex;
//    pub struct ConduitGroupmeToHvZ { syncer: groupme_syncer::GroupmeSyncer, scraper: hvz::HvZScraper }
//    impl ConduitGroupmeToHvZ {
//        pub fn new(group: groupme::Group) -> Self {
//            ConduitGroupmeToHvZ { syncer: groupme_syncer::GroupmeSyncer::new(group), scraper: hvz::HvZScraper::new() }
//        }
//    }
//    impl periodic::Periodic for ConduitGroupmeToHvZ {
//        fn tick(&mut self, _: usize) -> Result<()> {
//            Ok(())
//        }
//    }
//}
