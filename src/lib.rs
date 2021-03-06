#![recursion_limit = "2048"]
#![deny(warnings)]

extern crate chrono;
extern crate cookie;
#[macro_use] extern crate error_chain;
extern crate image;
/*#[macro_use]*/ extern crate itertools;
#[macro_use] extern crate lazy_static;
extern crate postgres;
extern crate rand;
extern crate regex;
extern crate reqwest;
extern crate rusttype;
extern crate scraper;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate serde;
#[macro_use(static_slice)] extern crate static_slice;
extern crate url;
extern crate uuid;
extern crate strum;
#[macro_use] extern crate strum_macros;
pub mod groupme;
pub mod hvz;
pub mod syncer;
pub mod render;

pub mod errors {
    #![allow(unused_doc_comment)]
    error_chain! {
        foreign_links {
            Reqwest (::reqwest::Error);
            Io (::std::io::Error);
            Postgres (::postgres::Error);
            SerdeJson (::serde_json::Error);
            UrlParsing (::url::ParseError);
            DateFormatParsing (::chrono::format::ParseError);
            EnvironmentVar (::std::env::VarError);
            EnumParseError (::strum::ParseError);
        }
        errors {
            RLE {}
            SignalHandlingThreadPanicked {
                description("A panic occurred in the signal-handling thread.")
            }
            HttpError(s: ::reqwest::StatusCode) {
                from()
                description("HTTP request returned OUTOFBAND error.")
                display("HTTP OUTOFBAND error: {:?}.", s)
            }
            TextTooLong(text: String, attachments: Option<Vec<::groupme::Attachment>>) {
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
            DbWriteNopped(k: String) {
                description("A database write failed to affect any rows.")
                display("Writing to key {:?} failed to affect any rows.", k)
            }
            NonEmptyResponse(s: String) {
                description("HTTP request unexpectedly non-empty.")
                display("HTTP request expected empty, got {:?}", s)
            }
            JsonTypeError(desc: &'static str) {
                description("JSON typing error")
                display("JSON typing error: {}", desc)
            }
            GroupOwnershipChangeFailed {
                description("Failed to change Group ownership.")
            }
            GroupRemovalFailed(who: ::groupme::Member) {
                description("Unable to find membership ID for an outgoing member.")
                display("Unable to find membership ID for outgoing member {:?}", who)
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
        pub fn new(username: String, password: String) -> Result<HvZSyncer> {
            let mut me = HvZSyncer { scraper: hvz::HvZScraper::new(username, password), conn: syncer::setup()?, killboard: hvz::Killboard::new(), chatboard: hvz::Chatboard::new(), panelboard: hvz::Panelboard::new(), };
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
            Ok(me)
        }
        pub fn update_killboard(&mut self) -> Result<Changes<hvz::Killboard>> {
            let (killboard, additions, deletions) = syncer::update_map(&self.conn, "killboard", self.scraper.fetch_killboard()?, &mut self.killboard, true)?;
            self.killboard = killboard;
            Ok((additions, deletions))
        }
        #[inline] pub fn new_zombies(&mut self) -> Result<Vec<hvz::Player>> { Ok(self.update_killboard()?.0.remove(&hvz::Faction::Human).unwrap_or(vec![])) }
        pub fn update_chatboard(&mut self) -> Result<Changes<hvz::Chatboard>> {
            let (chatboard, additions, deletions) = syncer::update_map(&self.conn, "chatboard", self.scraper.fetch_chatboard()?, &mut self.chatboard, true)?;
            self.chatboard = chatboard;
            Ok((additions, deletions))
        }
        pub fn update_panelboard(&mut self) -> Result<Changes<hvz::Panelboard>> {
            let (panelboard, additions, deletions) = syncer::update_map(&self.conn, "panelboard", self.scraper.fetch_panelboard()?, &mut self.panelboard, false)?;
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
        pub fn new(group: groupme::Group) -> Result<GroupmeSyncer> {
            let conn = syncer::setup()?;
            let last_message_id = syncer::read(&conn, (group.group_id.clone() + "last_message_id").as_str()).ok();
            Ok(GroupmeSyncer { group: group, last_message_id: last_message_id, members: vec![], conn: conn })
        }
        pub fn update_messages(&mut self) -> Result<Vec<groupme::Message>> {
            let last_message_id = self.last_message_id.clone();
            let selector = last_message_id.clone().map(groupme::MessageSelector::After);
            //let selector = match last_message_id {
            //    Some(ref m) => Some(groupme::MessageSelector::After(m.clone())),
            //    None => None,
            //};
            let ret = self.group.slurp_messages(selector.clone())?;
            self.last_message_id = if ret.len() > 0 { Some(ret[ret.len()-1].id.clone()) } else { last_message_id };
            if let Some(ref last_message_id) = self.last_message_id { syncer::write(&self.conn, (self.group.group_id.clone() + "last_message_id").as_str(), last_message_id.as_str())?; }
            if let Some(_) = selector { Ok(ret) } else { Ok(vec![]) }
        }
    }

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
    use self::chrono::{Timelike,TimeZone};
    use groupme;
    use groupme::{Recipient};
    use periodic;
    use rand;
    use rand::Rng;
    use std::convert::Into;
    use std::iter::FromIterator;
    use errors::*;
    use regex;
    use itertools::{Itertools};
    //use serde::{Serialize,Deserialize};
    use serde_json;

    lazy_static!{
        static ref KILLBOARD_CHECKERS : Vec<&'static str> = vec![ // TODO make these user-lists configurable ... somehow ...
"21806948" /* Milo Mirate */,
"21815306" /* Sriram Ganesan */,
"13153662" /* Josh Netter */,
        ];
    }

    fn entry_or_try_insert_with<'a, 'b: 'a, F: FnOnce(K) -> Result<V>, K: Ord + Copy, V>(this: &'b mut std::collections::BTreeMap<K, V>, key: K, default: F) -> Result<&'a mut V> {
        Ok(match this.entry(key) {
            std::collections::btree_map::Entry::Occupied(oe) => oe.into_mut(),
            std::collections::btree_map::Entry::Vacant(ve) => ve.insert(default(key)?),
        })
    }

    fn nll<S: std::fmt::Display + std::borrow::Borrow<str>>(items: Vec<S>, postlude: Option<&str>) -> String {
        if let Some((tail, init)) = items.split_last() {
            if let Some((_, _)) = init.split_last() {
                format!("{} and {}{}", init.join(", "), tail, postlude.unwrap_or(""))
            } else { format!("{}", tail) }
        } else { String::new() }
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
        fn _upsert(&self, factiongroup: &groupme::Group) -> Result<groupme::Bot> {
            groupme::Bot::upsert(factiongroup, self.nickname(), self.avatar_url(), None)
        }
        fn retrieve<'a>(&self, factiongroup: &groupme::Group, cache: &'a mut std::collections::BTreeMap<Self, groupme::Bot>) -> Result<&'a mut groupme::Bot> {
            entry_or_try_insert_with(cache, *self, |this| this._upsert(factiongroup))
        }
        fn nickname(&self) -> String {
            match self {
                &BotRole::VoxPopuli => "Vox Populi".to_string(),
                &BotRole::Chat(f) => capitalize(&format!("{:?} Chat", f)),
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
                &BotRole::VoxPopuli => "If certain people start their message with \"@Everyone\" or \"@everyone\", I'll repeat the message in such a way that it will \"@mention\" *everyone*.\nAbuse this, and there will be consequences.".to_string(),
                &BotRole::Chat(f) => format!("I'm the voice of {:?} chat. When someone posts something there, I'll tell you about it within a few seconds.{}", f, if f == hvz::Faction::General { " Except during gameplay hours, because spam is bad." } else { "" }),
                &BotRole::Killboard(hvz::Faction::Zombie) => "If someone shows up on the other side of the killboard, I'll report it here within a few minutes, and simultaneously try to go about kicking them. If I can't kick them, I'll give a holler.".to_string(),
                &BotRole::Killboard(hvz::Faction::Human) => "Whenever someone signs up, I'll report it here.".to_string(),
                &BotRole::Killboard(x) => format!("ERROR: THE \"{:?}\" FACTION LACKS A KILLBOARD SECTION", x),
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
                &BotRole::Killboard(hvz::Faction::Zombie) => messages!["Verdammt! We lost {} to {}. I hope it was worth it.", "(－‸ლ) {} died to {}. Come on; we can do better than this!", "Well, I'll be. Looks like {} bit the dust of {}.", "Well, drat. I think the zombies (namely, {1}) got {0}. I hope they died with dignity.", "Well, I declare. Seems that {} kicked the bucket, kudos to {}.", "Hunh. I guess that {1} turned {0} to the undeath. A grim inevitability.", "Are you kidding me? Killboard says {1} nommed {}! Fight harder!", "Argh. {} passed on to the undeath, with the help of {}."],
                &BotRole::Killboard(hvz::Faction::Human) => messages!["({:?}, {:?})"],
                &BotRole::Killboard(_) => messages!["{:?}{:?}"],
                &BotRole::Panel(hvz::PanelKind::Mission) => messages!["{:?}{:?}"],
                &BotRole::Panel(hvz::PanelKind::Announcement) => messages!["{:?}{:?}"],
            };
            (*rand::thread_rng().choose(formats).unwrap_or(&_f))(param1, param2)
        }
    }

    fn ts(m: &hvz::Message) -> String {
        let min = chrono::Local::now().signed_duration_since(m.timestamp).num_minutes();
        if min > 60 { m.timestamp.format(" [%a %H:%M:%S] ").to_string() }
        else if min > 2 { m.timestamp.format(" [%H:%M:%S] ").to_string() }
        else { " ".to_string() }
    }

    #[derive(Serialize, Deserialize)] struct RuntimeState { newbs: bool, dormant: bool, annxs: bool, missions: bool, throttled_at: i64 }
    impl RuntimeState {
        fn write(&self, cncgroup: &mut groupme::Group) -> Result<()> {
            cncgroup.update(None, serde_json::to_string(&self)?.into(), None, None).map(|_| ())
        }
    }
    impl Default for RuntimeState { fn default() -> Self { RuntimeState { newbs: false, dormant: true, annxs: false, missions: false, throttled_at: 0i64 } } }

    pub struct ConduitHvZToGroupme { factionsyncer: groupme_syncer::GroupmeSyncer, cncsyncer: groupme_syncer::GroupmeSyncer, hvz: hvz_syncer::HvZSyncer, bots: std::collections::BTreeMap<BotRole, groupme::Bot>, state: RuntimeState }
    impl ConduitHvZToGroupme {
        pub fn new(factiongroup: groupme::Group, mut cncgroup: groupme::Group, username: String, password: String) -> Result<Self> {
            let mut bots = std::collections::BTreeMap::new();
            for role in vec![BotRole::VoxPopuli, BotRole::Chat(hvz::Faction::Human), BotRole::Chat(hvz::Faction::General), BotRole::Killboard(hvz::Faction::Human), BotRole::Killboard(hvz::Faction::Zombie)/*, BotRole::Panel(hvz::PanelKind::Mission), BotRole::Panel(hvz::PanelKind::Announcement)*/].into_iter() {
                bots.insert(role, groupme::Bot::upsert(&factiongroup, role.nickname(), role.avatar_url(), None)?);
            }
            let tutorial = cncgroup.description.is_none();
            let state = match serde_json::from_str(cncgroup.description.as_ref().map(AsRef::as_ref).unwrap_or("")) {
                Ok(s) => s,
                _ => { let s = RuntimeState::default(); s.write(&mut cncgroup)?; s }
            };
            cncgroup.post(format!("<> bot starting up; in {} state. please say \"!wakeup\" to exit the dormant state, or \"!sleep\" to re-enter it.", if state.dormant { "DORMANT" } else { "ACTIVE" }), None)?;
            if tutorial {
                cncgroup.post("<> private annunciation of new players can be toggled at any time. to do so, say \"!newbs on\" or \"!newbs off\".".to_owned(), None)?;
                cncgroup.post("<> depending on how the admins are doing with \"Remind\", public annunciation of announcements and missions can be individually toggled at any time. to do so, say \"!annxs on\", \"!annxs off\", \"!missions on\" or \"!missions off\".".to_owned(), None)?;
                cncgroup.post("<> if the game has not yet started and you're not afraid of being accused of spam, say \"!mic check please\" to publicly post a list of features.".to_owned(), None)?;
            }
            Ok(ConduitHvZToGroupme { factionsyncer: groupme_syncer::GroupmeSyncer::new(factiongroup)?, cncsyncer: groupme_syncer::GroupmeSyncer::new(cncgroup)?, hvz: hvz_syncer::HvZSyncer::new(username, password)?, bots: bots, state: state })
        }
        pub fn mic_check(&mut self) -> Result<()> {
            for (role, bot) in self.bots.iter() {
                bot.post(role.watdo(), None)?;
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            for ((_, bot), message) in self.bots.iter().zip(vec!["Oh, and be careful. If our operator dies, so do we.", "In such an event, our code is hosted at https://github.com/mmirate/groupme_hvz_rs .", "Just throw it up on Heroku's free plan. (https://www.heroku.com)"]) {
                bot.post(message.to_string(), None)?;
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            self.factionsyncer.group.post("Two more things. (1) If you start your message with \"@Human Chat\" or \"@General Chat\", I'll repost it to the requested HvZ website chat.".to_string(), None)?;
            std::thread::sleep(std::time::Duration::from_secs(5));
            self.factionsyncer.group.post("(2) If your message includes the two words \"I'm dead\" adjacently and in that order (or \"I am dead\"; again, adjacently in that order), but without the doublequotes and regardless of capitalization or non-doublequote & non-questionmark punctuation, and with any number of certain adverbs (e.g. \"definitely\") ... then I will kick you from the Group within about half a minute. So please tell us you're dead and wait a minute; instead of immediately removing yourself.".to_string(), None)?;
            std::thread::sleep(std::time::Duration::from_secs(5));
            Ok(())
        }
        pub fn quick_mic_check(&mut self) -> Result<()> {
            let text = if self.state.dormant { "Okay, backup bot is up. Ping me if the current bot-operator dies." } else { "Okay, the bot is online." };
            self.factionsyncer.group.post(text.to_string(), None).map(|_| ())
        }

        fn make_headcount(&mut self) -> Result<()> {
            let (h, z) = (self.hvz.killboard.get(&hvz::Faction::Human).map(Vec::len).unwrap_or_default(), self.hvz.killboard.get(&hvz::Faction::Zombie).map(Vec::len).unwrap_or_default());
            let (f, message) = if z == 0 {
                (hvz::Faction::Human, format!("Thusfar we have recruited {} people to the cause of Humanity. Hail Victory!", h))
            } else {
                (hvz::Faction::Zombie, format!("You currently have {} Humans versus {} Zombies. Fight harder!", h, z))
            };
            BotRole::Killboard(f).retrieve(&self.factionsyncer.group, &mut self.bots)?.post(message, None)?;
            Ok(())
        }

        fn process_cnc_message(&mut self, message: groupme::Message) -> Result<()> {
            if !message.favorited_by.is_empty() { return Ok(()); }
            let _text = message.text.clone();
            let words = _text.split('!').nth(1).unwrap_or_default().split_whitespace().collect::<Vec<_>>();
            let mission_complete = if words == ["mic", "check", "please"] {
                let mc = self.cncsyncer.group.post("<> mic check; aye, aye".to_string(), None)?;
                message.like()?;
                self.mic_check()?;
                Some(mc)
            } else if words == ["heartbeat", "please"] {
                let mc = self.cncsyncer.group.post("<> quick mic check; aye, aye".to_string(), None)?;
                message.like()?;
                self.quick_mic_check()?;
                Some(mc)
            } else if words == ["headcount", "please"] {
                let mc = self.cncsyncer.group.post("<> headcount; aye, aye".to_string(), None)?;
                message.like()?;
                self.make_headcount()?;
                Some(mc)
            } else if words.get(0) == Some(&"wakeup") {
                let mc = self.cncsyncer.group.post("<> waking up; aye, aye. good luck, operator. say \"!heartbeat please\" if you wish to announce my awakening.".to_string(), None)?;
                message.like()?;
                self.state.dormant = false;
                self.state.write(&mut self.cncsyncer.group)?;
                Some(mc)
            } else if words.get(0) == Some(&"sleep") {
                let mc = self.cncsyncer.group.post("<> going to sleep; aye, aye".to_string(), None)?;
                message.like()?;
                self.state.dormant = true;
                self.state.write(&mut self.cncsyncer.group)?;
                Some(mc)
            } else if words.get(0) == Some(&"dead") {
                let mc = self.cncsyncer.group.post("<> going to sleep; aye, aye. may the horde be with you.".to_string(), None)?;
                message.like()?;
                self.state.dormant = true;
                self.state.write(&mut self.cncsyncer.group)?;
                Some(mc)
            } else if words.get(0).map(|s| s.starts_with("new")).unwrap_or(false) {
                match words.get(words.len()-1) {
                    Some(&"on") => {
                        let mc = self.cncsyncer.group.post("<> new player annunciation ON; aye, aye".to_string(), None)?;
                        message.like()?;
                        self.state.newbs = true;
                        self.state.write(&mut self.cncsyncer.group)?;
                        Some(mc)
                    },
                    Some(&"off") => {
                        let mc = self.cncsyncer.group.post("<> new player annunciation OFF; aye, aye".to_string(), None)?;
                        message.like()?;
                        self.state.newbs = false;
                        self.state.write(&mut self.cncsyncer.group)?;
                        Some(mc)
                    },
                    _ => None,
                }
            } else if words.get(0).map(|s| s.starts_with("annx")).unwrap_or(false) {
                match words.get(words.len()-1) {
                    Some(&"on") => {
                        let mc = self.cncsyncer.group.post("<> announcement annunciation ON; aye, aye".to_string(), None)?;
                        message.like()?;
                        self.state.annxs = true;
                        self.state.write(&mut self.cncsyncer.group)?;
                        Some(mc)
                    },
                    Some(&"off") => {
                        let mc = self.cncsyncer.group.post("<> announcement annunciation OFF; aye, aye".to_string(), None)?;
                        message.like()?;
                        self.state.annxs = false;
                        self.state.write(&mut self.cncsyncer.group)?;
                        Some(mc)
                    },
                    _ => None,
                }
            } else if words.get(0).map(|s| s.starts_with("mission")).unwrap_or(false) {
                match words.get(words.len()-1) {
                    Some(&"on") => {
                        let mc = self.cncsyncer.group.post("<> mission annunciation ON; aye, aye".to_string(), None)?;
                        message.like()?;
                        self.state.missions = true;
                        self.state.write(&mut self.cncsyncer.group)?;
                        Some(mc)
                    },
                    Some(&"off") => {
                        let mc = self.cncsyncer.group.post("<> mission annunciation OFF; aye, aye".to_string(), None)?;
                        message.like()?;
                        self.state.missions = false;
                        self.state.write(&mut self.cncsyncer.group)?;
                        Some(mc)
                    },
                    _ => None,
                }
            } else { None };
            if let Some(mc) = mission_complete { mc.like()?; }
            Ok(())
        }

        fn process_cnc(&mut self, _i: usize) -> Result<()> {
            self.cncsyncer.update_messages()?.into_iter().map(|x| self.process_cnc_message(x)).collect::<Vec<_>>().into_iter().collect::<Result<Vec<_>>>().map(drop)
        }

        fn process_groupme_message(&mut self, me: &groupme::User, now: &chrono::DateTime<chrono::Local>, message: groupme::Message) -> Result<()> {
            lazy_static!{
                static ref MESSAGE_TO_HVZCHAT_RE: regex::Regex = regex::Regex::new(r"^@(?P<faction>(?:[Gg]en(?:eral)?|[Aa]ll)|(?:[Hh]um(?:an)?)|(?:[Zz]omb(?:ie)?))(?: |-)?(?:[Cc]hat)? (?P<message>.+)").unwrap();
                static ref MESSAGE_TO_EVERYONE_RE: regex::Regex = regex::Regex::new(r"^@[Ee]veryone (?P<message>.+)").unwrap();
                static ref MESSAGE_TO_ADMINS_RE: regex::Regex = regex::Regex::new(r"^@[Aa]dmins (?P<message>.+)").unwrap();
                static ref I_AM_DEAD_RE: regex::Regex = regex::Regex::new(r" i( a)m ((very|quite|definitely|totally|100%|completely|acutely|grievously|severely|regrettably|no-(joke|shit|troll)|thoroughly|absolutely|clearly|decidedly|doubtlessly|finally|obviously|plainly|certainly|undeniably|unequivocally|unquestionably|indubitably|positively|unmistakably) )*dead ").unwrap();
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
                static ref THE_ADMINS: Vec<&'static str> = vec![
"8856552" /* ??? */,
"20298305" /* ??? */,
"19834407" /* ??? */,
"12949596" /* ??? */,
"13094442" /* ??? */,
                ];
            }

            if 7 <= now.hour() && now.hour() < 23 && now.signed_duration_since(chrono::Local.timestamp(message.created_at as i64, 0)).num_hours() <= 6 {
                let signature = format!(" {} ", message.text.to_lowercase().split_whitespace().map(|word| word.replace(|c: char| { !c.is_alphabetic() && c != '"' && c != '?' }, "")).collect::<Vec<String>>().join(" "));
                if I_AM_DEAD_RE.is_match(&signature) {
                    if !([&self.factionsyncer.group.creator_user_id, &me.user_id].contains(&&message.user_id)) {
                        if let Some(member) = self.factionsyncer.group.members.iter().find(|&m| m.user_id == message.user_id) {
                            if let Err(_) = self.factionsyncer.group.remove(member.clone()) {
                                //self.factionsyncer.group.post("... I guess they already got kicked?".to_owned(), None).map(|_| ())? // Actually, we don't care about this.
                            }
                        }
                        self.factionsyncer.group.refresh()?;
                    }
                }
            }
            if self.state.dormant { return Ok(()); }
            if let Some(cs) = MESSAGE_TO_EVERYONE_RE.captures(message.text.as_str()) {
                if ALLOWED_MESSAGEBLASTERS.contains(message.user_id.as_str()) {
                    if let Some(m) = cs.name("message") {
                        let vox = BotRole::VoxPopuli.retrieve(&self.factionsyncer.group, &mut self.bots)?;
                        vox.post_mentioning(format!("[{}] {}", message.name, m.as_str()), self.factionsyncer.group.member_uids_except(message.user_id.as_str()), None)?;

                    }
                }
            } else if let Some(cs) = MESSAGE_TO_HVZCHAT_RE.captures(message.text.as_str()) {
                if let (Some(f), Some(m)) = (cs.name("faction"), cs.name("message")) {
                    self.hvz.scraper.post_chat(f.as_str().to_lowercase().parse()?, format!("[{} from GroupMe] {}", message.name, m.as_str()).as_str())?;
                }
            } else if let Some(cs) = MESSAGE_TO_ADMINS_RE.captures(message.text.as_str()) { // TODO REDO
                if let Some(m) = cs.name("message") {
                    let vox = BotRole::VoxPopuli.retrieve(&self.factionsyncer.group, &mut self.bots)?;
                    vox.post_mentioning(m.as_str(), THE_ADMINS.iter().cloned(), None)?;
                    //self.hvz.scraper.post_chat(hvz::Faction::Human, format!("@admins {} from GroupMe says, {:?}.", message.name, m).as_str())?;

                }
            }
            Ok(())
        }

        fn process_groupme_messages(&mut self, _i: usize) -> Result<()> {
            let now = chrono::Local::now();
            let me = groupme::User::get()?;
            self.factionsyncer.update_messages()?.into_iter().map(|x| self.process_groupme_message(&me, &now, x)).collect::<Vec<_>>().into_iter().collect::<Result<Vec<_>>>().map(drop)
        }

        fn mark_zombie(&self, zombie_player: hvz::Player) -> std::result::Result<(hvz::Player,groupme::Member),hvz::Player> {
            for member in self.factionsyncer.group.members.iter() {
                if member.canonical_name().to_lowercase() == zombie_player.playername.to_lowercase() {
                    return Ok((zombie_player, member.to_owned()));
                }
            }
            if self.hvz.killboard.name_has_ambiguous_surname(&zombie_player.playername) {
                Err(zombie_player)
            } else {
                for member in self.factionsyncer.group.members.iter() {
                    if hvz::Killboard::surname(&member.canonical_name()).to_lowercase() == hvz::Killboard::surname(&zombie_player.playername).to_lowercase() {
                        return Ok((zombie_player, member.to_owned()));
                    }
                }
                Err(zombie_player)
            }
        }

        fn process_new_zombies(&mut self, new_zombies: Vec<hvz::Player>) -> Result<()> {
            self.factionsyncer.group.refresh()?;
            let (not_found, to_remove) : (Vec<_>, Vec<(_,_)>) =
                new_zombies.into_iter().partition_map(|z| self.mark_zombie(z).into());
            let not_kicked : Vec<(_,_)> = to_remove.into_iter().filter_map(|(death, removal)| {
                if let Err(Error(ErrorKind::GroupRemovalFailed(rem), _)) = self.factionsyncer.group.remove(removal.clone()) {
                    Some((death, rem))
                } else { None }
            }).collect();
            self.factionsyncer.group.refresh()?;
            if !not_found.is_empty() || !not_kicked.is_empty() {
                let role = BotRole::Killboard(hvz::Faction::Zombie);
                let bot = role.retrieve(&self.factionsyncer.group, &mut self.bots)?;
                let has_not_found = !not_found.is_empty();
                let has_not_kicked = !not_kicked.is_empty();
                let not_found_names = "failed to ID (let alone kick) ".to_owned() + nll(not_found.iter().map(|p| p.playername.as_str()).collect::<Vec<_>>(), None).as_str();
                let not_kicked_names = "failed to kick ".to_owned() + nll(not_kicked.into_iter().map(|(p, m)| format!("{} (aka {:?})", p.playername, m.nickname)).collect::<Vec<_>>(), None).as_str();
                let mut warnings : Vec<&str> = vec![];
                if has_not_found { warnings.push(&not_found_names); }
                if has_not_kicked { warnings.push(&not_kicked_names); }
                bot.post_mentioning(format!("Achtung; Trottel! Of those deaths, I {}.", nll(warnings, None)), KILLBOARD_CHECKERS.clone(), None)?;
            }
            Ok(())
        }

        fn process_killboard(&mut self, i: usize) -> Result<()> {
            let (hour, _minute) = { let n = chrono::Local::now(); (n.hour(), n.minute()) };
            if 2 < hour && hour < 7 { return Ok(()); }
            if i % 8 == 0 {
                let (additions, _deletions) = self.hvz.update_killboard()?;
                for (faction, new_members) in additions.into_iter() {
                    if new_members.is_empty() { continue; }
                    let role = BotRole::Killboard(faction);
                    if let BotRole::Killboard(hvz::Faction::Human) = role {
                        if !self.state.newbs { continue; }
                        for member in new_members.iter() {
                            self.cncsyncer.group.post(format!("<> new player: {}@gatech.edu - {}", member.gtname, member.playername), None)?;
                        }
                        continue;
                    }
                    if self.state.dormant { continue; }
                    {
                        let new_member_names = new_members.iter().map(|p| p.playername.as_str()).collect::<Vec<&str>>();
                        let m = match new_members.iter().map(|p| { p.kb_playername(&self.hvz.killboard.get(&hvz::Faction::Zombie).unwrap_or(&vec![])).map(String::from) }).collect::<Option<Vec<String>>>() {
                            Some(perpetrators) => role.phrase_2(nll(new_member_names, None).as_str(), nll(perpetrators.iter().map(AsRef::as_ref).collect(), " (resp.)".into()).as_str()),
                            None => role.phrase(nll(new_member_names, None).as_str())
                        };
                        role.retrieve(&self.factionsyncer.group, &mut self.bots)?.post(m, None)?;
                    }
                    if faction == hvz::Faction::Zombie {
                        self.process_new_zombies(new_members)?;
                    }
                }
            }
            Ok(())
        }

        fn process_panelboard(&mut self, i: usize) -> Result<()> {
            let (now, hour, minute) = { let n = chrono::Local::now(); (n, n.hour(), n.minute()) };
            if 2 < hour && hour < 7 { return Ok(()); }
            if (15 - ((minute as i32)%30)).abs() >= 12 && i % 4 == 2 /*i % 6 == 1*/ {
                let (additions, _deletions) = self.hvz.update_panelboard()?;
                for (kind, new_panels) in additions.into_iter() {
                    if self.state.dormant { continue; }
                    if ! ({ match kind { hvz::PanelKind::Announcement => self.state.annxs, hvz::PanelKind::Mission => self.state.missions } }) { continue; }
                    if new_panels.is_empty() { continue; }
                    let role = BotRole::Panel(kind);
                    let bot = role.retrieve(&self.factionsyncer.group, &mut self.bots)?;
                    for panel in new_panels.into_iter() {
                        if kind == hvz::PanelKind::Mission && panel.particulars.map(|p| (p.start.signed_duration_since(now)).num_minutes()).map(|m| m > 65 || m < 15).unwrap_or(true) { continue; } // only post about missions that are actually upcoming
                        bot.post_mentioning(format!("{}\n{}", role.phrase(panel.title.as_str()), panel.text), self.factionsyncer.group.member_uids(), None)?;
                    }
                }
            }
            Ok(())
        }

        fn process_chatboard(&mut self, i: usize) -> Result<()> {
            let (now, hour, _minute) = { let n = chrono::Local::now(); (n, n.hour(), n.minute()) };
            if 2 < hour && hour < 7 { return Ok(()); }
            if i % 8 == 1 || (!self.state.dormant && i % 4 == 1) {
                let (additions, _deletions) = self.hvz.update_chatboard()?;
                for (faction, new_messages) in additions.into_iter() {
                    if new_messages.is_empty() { continue; }
                    if faction == hvz::Faction::General && 7 <= hour && hour < 23 { continue; }
                    if self.state.dormant { continue; }
                    let role = BotRole::Chat(faction);
                    let bot = role.retrieve(&self.factionsyncer.group, &mut self.bots)?;
                    for message in new_messages.into_iter() {
                        if (now.signed_duration_since(message.timestamp)).num_hours().abs() > 24 { continue; }
                        bot.post(format!("[{}]{}{}", message.sender.playername, ts(&message), message.text), None)?;
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
                        ret.push(bot.post_mentioning("Ouch. We're being throttled, so we have to go down for 15 minutes. Please check the killboard while we're gone!".to_owned(), KILLBOARD_CHECKERS.clone(), None).map(drop));
                        //ret.push(bot.post(format!("{: <1$}", "Ouch. We're being throttled, so we have to go down for 15 minutes. Please check the killboard while we're gone!", KILLBOARD_CHECKERS.len()), Some(vec![self.factionsyncer.group.mention_ids(&KILLBOARD_CHECKERS)])).map(drop));
                        ret.into_iter().collect::<Result<Vec<()>>>().map(|_: Vec<_>| ())
                    }
                } else { eprintln!("HOLY S*** I JUST ATE SOME DATA! HOW DO WE HAVE LITERALLY NO BOTS WHATSOEVER?!"); e }
            }
        }

    }
    impl periodic::Periodic for ConduitHvZToGroupme {
        fn tick(&mut self, i: usize) -> Result<()> {
            // non-use of "?" is intentional here; we want to attempt each function even if one fails
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
