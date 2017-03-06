use chrono;
use hyper;
use scraper;
use url;
use std;
use std::iter::FromIterator;
use std::collections::BTreeMap;
use chrono::{TimeZone,Datelike};
use errors::*;

//fn unwrap<T,E: std::fmt::Debug>(r: std::result::Result<T,E>) -> T { r.unwrap() }

#[inline] fn form_type() -> hyper::header::ContentType { hyper::header::ContentType(hyper::mime::Mime(hyper::mime::TopLevel::Application, hyper::mime::SubLevel::WwwFormUrlEncoded,vec![(hyper::mime::Attr::Charset,hyper::mime::Value::Utf8)])) }

pub type Killboard = BTreeMap<Faction, Vec<Player>>;
pub type Chatboard = BTreeMap<Faction, Vec<Message>>;
pub type Panelboard = BTreeMap<PanelKind, Vec<Panel>>;

fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> { v.sort(); v }

#[derive(Copy, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(RustcEncodable, RustcDecodable)] pub enum Faction { General, Human, Zombie, Admin }
impl Default for Faction { fn default() -> Faction { Faction::General } }
impl Faction {
    #[inline] pub fn killboards() -> Vec<Faction> { vec![Faction::Human, Faction::Zombie] }
    #[inline] pub fn chats() -> Vec<Faction> { vec![Faction::General, Faction::Human, Faction::Zombie] }
    #[inline] pub fn populated() -> Vec<Faction> { vec![Faction::Admin, Faction::Human, Faction::Zombie] }
}
impl std::fmt::Display for Faction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { Faction::Human => "human", Faction::Zombie => "zombie", Faction::Admin => "admin", Faction::General => "general", }) }
}
impl std::fmt::Debug for Faction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { Faction::Human => "hum", Faction::Zombie => "zomb", Faction::Admin => "admin", Faction::General => "all", }) }
}
impl std::fmt::Binary for Faction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { Faction::Human => "h", Faction::Zombie => "z", Faction::Admin => "a", Faction::General => "?" }) }
}
impl<'b> From<&'b str> for Faction { fn from(s: &'b str) -> Faction { match s.to_lowercase().as_str() {
    "human" | "hum" | "h" => Faction::Human, "zombie" | "zomb" | "z" => Faction::Zombie, "admin" => Faction::Admin, _ => Faction::General,
} } }

use std::cmp::Ordering;

#[derive(Clone, Debug, Hash)] #[derive(RustcEncodable, RustcDecodable)] pub struct Player { pub gtname: String, pub playername: String, pub faction: Faction, pub kb_playername: Option<String> }
impl PartialEq for Player { fn eq(&self, other: &Player) -> bool { (&self.gtname, &self.playername) == (&other.gtname, &other.playername) } }
impl Eq for Player {}
impl Ord for Player { fn cmp(&self, other: &Player) -> Ordering { (&self.playername, &self.gtname).cmp(&(&other.playername, &other.gtname)) } }
impl PartialOrd for Player { fn partial_cmp(&self, other: &Player) -> Option<Ordering> { Some(self.cmp(other)) } }
//impl Player { pub fn erase(&self) -> Player { Player { faction: Faction::General, .. self.clone() } } }
impl Player {
    pub fn from_document(doc: scraper::Html) -> Result<Self> {
        let (faction_selector, playername_selector) = (try!(scraper::Selector::parse("div.page-header > h3").map_err(|()| Error::from(ErrorKind::CSS))), try!(scraper::Selector::parse("div.page-header > h1").map_err(|()| Error::from(ErrorKind::CSS))));
        Ok(Player {
            gtname: String::new(),
            playername: try!(doc.select(&playername_selector).next().ok_or(Error::from(ErrorKind::Scraper("doc->Player.playername")))).inner_html().trim().to_string(),
            faction: Faction::from(try!(doc.select(&faction_selector).next().ok_or(Error::from(ErrorKind::Scraper("doc->Player.faction")))).inner_html().trim()),
            kb_playername: None
        })
    }
    pub fn from_kb_link<'a>(link: scraper::ElementRef<'a>) -> Result<Self> {
        //let p_selector = try!(scraper::Selector::parse("p").map_err(|()| Error::from(ErrorKind::CSS)));
        Ok(Player {
            gtname: try!(try!(try!(url::Url::parse("https://hvz.gatech.edu/killboard")).join(try!(link.value().attr("href").ok_or(Error::from(ErrorKind::Scraper("kb->link->href:Player.gtname")))))).query_pairs().next().ok_or(Error::from(ErrorKind::Scraper("join(kb->link->href):Player.gtname")))).1.to_string(),
            playername: link.text().collect::<Vec<_>>().concat().trim().to_owned(),
            faction: Faction::default(),
            kb_playername: link.parent().and_then(|td| td.next_sibling()).and_then(|td| td.next_sibling()).and_then(scraper::ElementRef::wrap).map(|td| td.text().collect::<Vec<_>>().concat().trim().to_owned())
        })
    }
    pub fn from_chat_link<'a>(link: scraper::ElementRef<'a>) -> Result<Self> {
        //let p_selector = try!(scraper::Selector::parse("p").map_err(|()| Error::from(ErrorKind::CSS)));
        Ok(Player {
            gtname: try!(try!(try!(url::Url::parse("https://hvz.gatech.edu/killboard")).join(try!(link.value().attr("href").ok_or(Error::from(ErrorKind::Scraper("kb->link->href:Player.gtname")))))).query_pairs().next().ok_or(Error::from(ErrorKind::Scraper("join(kb->link->href):Player.gtname")))).1.to_string(),
            playername: link.text().collect::<Vec<_>>().concat().trim().to_owned(),
            faction: Faction::default(),
            kb_playername: None
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(RustcEncodable, RustcDecodable)] pub struct Message { pub timestamp: chrono::DateTime<chrono::Local>, pub sender: Player, pub receiver: Faction, pub text: String, }
impl Message { pub fn from_tr<'a>(tr: scraper::ElementRef<'a>) -> Result<Self> {
    let col_selector = try!(scraper::Selector::parse("td").map_err(|()| Error::from(ErrorKind::CSS)));
    let link_selector = try!(scraper::Selector::parse("a[href*=\"gtname\"]").map_err(|()| Error::from(ErrorKind::CSS)));
    let cols : Vec<scraper::ElementRef> = tr.select(&col_selector).collect();
    Ok(Message{sender: try!(Player::from_chat_link(try!(cols[0].select(&link_selector).next().ok_or(Error::from(ErrorKind::Scraper("chat->tr->link:Player")))))), timestamp: try!(chrono::Local.datetime_from_str(&format!("{}/{}", chrono::Local::today().year(), cols[1].inner_html().trim()), "%Y/%m/%d %H:%M")), receiver: Faction::default(), text: cols[2].inner_html().trim().to_owned()})
} }

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(RustcEncodable, RustcDecodable)] pub enum PanelKind { Announcement, Mission }
impl std::fmt::Display for PanelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { PanelKind::Announcement => "announcement", PanelKind::Mission => "mission", }) }
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(RustcEncodable, RustcDecodable)] pub struct MissionWW { pub start: chrono::DateTime<chrono::Local>, pub end: chrono::DateTime<chrono::Local>, pub location: String }
impl MissionWW {
    pub fn from_p<'a>(p: scraper::ElementRef<'a>) -> Option<MissionWW> {
        let d = BTreeMap::from_iter(p.text().collect::<Vec<_>>().concat().lines().map(str::trim).filter(|s| !s.is_empty()).filter_map(|l| {
            let mut it = l.splitn(2, ": ");
            if let Some(x) = it.next() {
                if let Some(y) = it.next() {
                    if let None = it.next() {
                        return Some((x.to_lowercase(), y.to_string()));
                    }
                }
            }
            return None;
        }));

        if let (Some(start), Some(end), Some(location)) = (d.get("start"), d.get("end"), d.get("location")) {
            if let (Ok(start), Ok(end)) = (Self::parse_mp_datetime(start), Self::parse_mp_datetime(end)) {
                Some(MissionWW { start: start, end: end, location: location.to_string() })
            } else { None }
        } else { None }
    }
    fn parse_mp_datetime(s: &str) -> Result<chrono::DateTime<chrono::Local>> {
        let (isoyear, isoweek, _) = chrono::Local::today().isoweekdate();
        let mut partial = chrono::format::Parsed::new();
        try!(partial.set_isoyear(isoyear as i64));
        try!(partial.set_isoweek(isoweek as i64));
        try!(chrono::format::parse(&mut partial, s, chrono::format::StrftimeItems::new("%a %H:%M")));
        partial.to_datetime_with_timezone(&chrono::Local).map_err(From::from)
    }
}
#[derive(Clone, Debug)] #[derive(RustcEncodable, RustcDecodable)] pub struct Panel { pub kind: PanelKind, pub title: String, pub particulars: Option<MissionWW>, pub text: String }
impl PartialEq for Panel { fn eq(&self, other: &Panel) -> bool {
    if let (&Some(_), &Some(_)) = (&self.particulars, &other.particulars) { (&self.kind, &self.particulars) == (&other.kind, &other.particulars) } else { (&self.kind, &self.title) == (&other.kind, &other.title) }
} }
impl Eq for Panel {}
impl Ord for Panel { fn cmp(&self, other: &Panel) -> Ordering {
    if let (&Some(_), &Some(_)) = (&self.particulars, &other.particulars) { (&self.kind, &self.particulars).cmp(&(&other.kind, &other.particulars)) } else { (&self.kind, &self.title).cmp(&(&other.kind, &other.title)) }
} }
impl PartialOrd for Panel { fn partial_cmp(&self, other: &Panel) -> Option<Ordering> { Some(self.cmp(other)) } }
impl Panel { pub fn from_div<'a>(div: scraper::ElementRef<'a>) -> Result<Panel> {
    let title_selector = try!(scraper::Selector::parse(".panel-title").map_err(|()| Error::from(ErrorKind::CSS)));
    let body_selector = try!(scraper::Selector::parse(".panel-body").map_err(|()| Error::from(ErrorKind::CSS)));
    let partics_selector = try!(scraper::Selector::parse("p.mission_particulars").map_err(|()| Error::from(ErrorKind::CSS)));
    Ok(Panel {
        kind: PanelKind::Announcement,
        title: try!(div.select(&title_selector).next().ok_or(Error::from(ErrorKind::Scraper("Panel.title")))).text().collect::<Vec<_>>().concat().trim().to_owned(),
        particulars: div.select(&partics_selector).next().and_then(MissionWW::from_p),
        text: try!(div.select(&body_selector).next().ok_or(Error::from(ErrorKind::Scraper("Panel.text")))).text().collect::<Vec<_>>().concat().trim().to_owned()
    })
} }

pub trait KillboardExt {
    fn surname(name: &str) -> &str;
    fn name_has_ambiguous_surname(&self, fullname: &str) -> bool;
}

impl KillboardExt for Killboard {
    fn surname(name: &str) -> &str { name.split_whitespace().last().unwrap_or("") }
    fn name_has_ambiguous_surname(&self, fullname: &str) -> bool {
        let needle = Self::surname(fullname);
        self.values().map(|players| players.iter().find(|p| needle.to_lowercase() == Self::surname(&p.playername).to_lowercase() && p.playername != fullname)).find(Option::is_some).is_some()
    }
}

type MyCookieJar = BTreeMap<String, hyper::header::CookiePair>;

#[derive(Clone, Debug)] pub struct HvZScraper { cookiejar: MyCookieJar, last_login: std::time::Instant, username: String, password: String }

impl HvZScraper {
    pub fn new(username: String, password: String) -> HvZScraper { HvZScraper { cookiejar: MyCookieJar::new(), last_login: std::time::Instant::now() - std::time::Duration::from_secs(1200)/*, client: hyper::client::Client::new()*/, username: username, password: password } }
    fn read_cookies<'b>(&self, rb: hyper::client::RequestBuilder<'b>) -> hyper::client::RequestBuilder<'b> {
        let h = hyper::header::Cookie(self.cookiejar.values().cloned().collect());
        //println!("Cookie: {:?}", h);
        rb.header(h)
    }
    fn write_cookies(&mut self, res: hyper::client::response::Response) -> hyper::client::response::Response {
        //println!("Set-Cookie: {:?}", res.headers.get::<hyper::header::SetCookie>());
        res.headers.get::<hyper::header::SetCookie>().map(|j| j.0.iter().map(|c : &hyper::header::CookiePair| { self.cookiejar.remove(&c.name); self.cookiejar.insert(c.name.clone(), c.clone()) }).last()); res
    }
    fn _slurp<R: std::io::Read>(mut r: R) -> std::io::Result<String> { let mut buffer = Vec::<u8>::new(); r.read_to_end(&mut buffer).and_then(|_| Ok(String::from_utf8_lossy(&buffer).to_string())) }
    #[inline] fn slurp(res: hyper::client::response::Response) -> hyper::Result<String> { Self::_slurp(res).map_err(hyper::error::Error::Io) }
    fn do_with_cookies<'b>(&mut self, rb: hyper::client::RequestBuilder<'b>, canfail: bool) -> Result<hyper::client::response::Response> {
        self.read_cookies(rb).send().map(|res| self.write_cookies(res)).map_err(Error::from).and_then(|res| {
            if res.status == hyper::status::StatusCode::Unregistered(509) {
                bail!(ErrorKind::BandwidthLimitExceeded)
            }
            if res.status.is_success() || canfail { Ok(res) } else {
                println!("HVZ-FACING REQUEST FAILED: {:?}", res);
                bail!(ErrorKind::HttpError(res.status))
            }
        })
    }
    fn _redirect_url(res: &hyper::client::response::Response) -> Option<url::Url> {
        match res.headers.get::<hyper::header::Location>() { Some(&hyper::header::Location(ref loc)) => { res.url.join(loc).ok() }, _ => None }
    }
    fn _fill_login_form(&self, doc: scraper::Html) -> Result<(String, url::Url)> {
        let form_selector = try!(scraper::Selector::parse("form").map_err(|()| Error::from(ErrorKind::CSS)));
        let form_control_selector = try!(scraper::Selector::parse("form input[name][value]").map_err(|()| Error::from(ErrorKind::CSS)));
        let mut querystring = url::form_urlencoded::Serializer::new(String::new());
        let querystring = querystring.append_pair("username", self.username.as_str()).append_pair("password", self.password.as_str());
        for e in doc.select(&form_control_selector) {
            if !(e.value().attr("type").map(|t| ["reset", "checkbox"].contains(&t)).unwrap_or(true))
                && !(["username", "password"].contains(e.value().attr("name").as_ref().unwrap_or(&""))) {
                querystring.append_pair(
                    try!(e.value().attr("name" ).ok_or(Error::from(ErrorKind::Scraper("login->form->element[name]" )))),
                    try!(e.value().attr("value").ok_or(Error::from(ErrorKind::Scraper("login->form->element[value]"))))
                );
            }
        }
        let querystring = querystring.finish();
        let u = try!(try!(url::Url::parse("https://login.gatech.edu/")).join(try!(try!(doc.select(&form_selector).next().ok_or(Error::from(ErrorKind::Scraper("login->form")))).value().attr("action").ok_or(Error::from(ErrorKind::Scraper("login->form[action]"))))));/*"https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules"*/
        Ok((querystring, u))
    }
    pub fn login(&mut self) -> Result<hyper::client::Client> {
        let mut client = hyper::client::Client::new();
        if self.last_login.elapsed() < std::time::Duration::from_secs(600) { return Ok(client); }
        println!("Cached login is old; refreshing session.");
        let res = try!(self.do_with_cookies(client.get(/*"https://hvz.gatech.edu/rules/"*/"https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f"), true));
        client.set_redirect_policy(hyper::client::RedirectPolicy::FollowNone);
        if res.url.host_str().unwrap_or("") != "hvz.gatech.edu" {
            let login_page = if res.url.host_str().map(|h| h == "login.gatech.edu").unwrap_or(false) {
                try!(Self::slurp(res))
            } else {
                try!(Self::slurp(try!(self.do_with_cookies(client.get("https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f"), false))))
            };
            let (body, u) = try!(self._fill_login_form(scraper::Html::parse_document(login_page.as_str())));
            let mut res = try!(self.do_with_cookies(client.post(u.as_str()).body(&body).header(form_type()), true));
            while res.url.host_str().unwrap_or("") != "hvz.gatech.edu" {
                if let Some(loc) = Self::_redirect_url(&res) { if loc.host_str().unwrap_or("") == "hvz.gatech.edu" { break; } }
                let document = scraper::Html::parse_document(try!(Self::slurp(try!(self.do_with_cookies(client.get("https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f"), true)))).as_str());
                let (body, u) = try!(self._fill_login_form(document));
                res = try!(self.do_with_cookies(client.post(u.as_str()).body(&body).header(form_type()), true));
                if !(res.status.is_success() || res.status.is_redirection()) {
                    bail!(ErrorKind::GaTechCreds);
                }
            }
            while let Some(loc) = Self::_redirect_url(&res) {
                res = try!(self.do_with_cookies(client.get(loc), true));
                if !(res.status.is_success() || res.status.is_redirection()) {
                    bail!(ErrorKind::GaTechCreds);
                }
            }
        }
        if try!(self.do_with_cookies(client.get("https://hvz.gatech.edu/rules/"), true)).url.host_str().unwrap_or("") != "hvz.gatech.edu" {
            bail!(ErrorKind::GaTechCreds);
        }
        client.set_redirect_policy(hyper::client::RedirectPolicy::FollowAll);
        self.last_login = std::time::Instant::now();
        Ok(client)
    }
    pub fn whois(&mut self, gtname: &str) -> Result<Player> {
        let client = try!(self.login());
        Player::from_document(scraper::Html::parse_document(try!(Self::slurp(try!(self.do_with_cookies(client.get(&format!("https://hvz.gatech.edu/profile/?gtname={}", gtname)), false)))).as_str()))
    }
    fn shrink_to_fit<T>(mut v: Vec<T>) -> Vec<T> { v.shrink_to_fit(); v }
    #[inline] pub fn whoami(&mut self) -> Result<Player> { let u = self.username.clone(); self.whois(u.as_str()) }
    #[inline] fn trace<T: std::fmt::Debug>(x: T) -> T { /* println!("{:?}", x); */ x }
    pub fn fetch_killboard(&mut self) -> Result<Killboard> {
        let client = try!(self.login());

        let doc = scraper::Html::parse_document(Self::trace(try!(Self::slurp({
            let mut res = try!(self.do_with_cookies(client.get("https://hvz.gatech.edu/killboard/"), false));
            while !res.url.as_str().contains("killboard") {
                println!("Capstoneurs...");
                res = try!(self.do_with_cookies(client.get("https://hvz.gatech.edu/killboard/"), false));
            }
            res
        })).as_str()));

        let mut ret = Killboard::new();
        for faction in Faction::killboards() {
            ret.remove(&faction);
            ret.insert(faction, sorted(try!(doc.select(&try!(scraper::Selector::parse(&Self::trace(format!("#{:b}killboard a[href*=\"gtname\"]", &faction))).map_err(|()| Error::from(ErrorKind::CSS)))).map(|link| { Ok(Player { faction: faction, .. try!(Player::from_kb_link(link)) }) }).collect::<Result<Vec<_>>>())));
        }
        Ok(ret)
    }
    pub fn fetch_chatboard(&mut self) -> Result<Chatboard> {
        let client = try!(self.login());

        let row_selector = try!(scraper::Selector::parse("tr.chat_line").map_err(|()| Error::from(ErrorKind::CSS)));
        //let col_selector = try!(scraper::Selector::parse("td").map_err(|()| Error::from(ErrorKind::CSS));
        //let link_selector = try!(scraper::Selector::parse("a[href*=\"gtname\"]").map_err(|()| Error::from(ErrorKind::CSS)));

        let mut ret = Chatboard::new();
        for faction in Faction::chats() {
            if faction != Faction::General && faction != try!(self.whoami()).faction { continue; }
            ret.remove(&faction);
            ret.insert(faction, Self::shrink_to_fit(try!(scraper::Html::parse_fragment(try!(Self::slurp(try!(self.do_with_cookies(client.post("https://hvz.gatech.edu/chat/_update.php").body(&format!("aud={:?}", faction)).header(form_type()), false)))).as_str()).select(&row_selector).map(|tr| Ok(Message { receiver: faction, .. try!(Message::from_tr(tr)) })).collect::<Result<Vec<_>>>())));
        }
        Ok(ret)
    }
    pub fn fetch_panelboard(&mut self) -> Result<Panelboard> {
        let client = try!(self.login());
        let mut ret = Panelboard::new();
        for kind in vec![/*PanelKind::Announcement, */PanelKind::Mission] {
            ret.remove(&kind);
            ret.insert(kind, Self::shrink_to_fit(try!(scraper::Html::parse_document(try!(Self::slurp(try!(self.do_with_cookies(client.get(&format!("https://hvz.gatech.edu/{}s", kind)), false)))).as_str()).select(&try!(scraper::Selector::parse(&format!("div.panel.{}", kind)).map_err(|()| Error::from(ErrorKind::CSS)))).map(|div| Ok(Panel { kind: kind, .. try!(Panel::from_div(div)) })).collect::<Result<Vec<_>>>())));
        }
        Ok(ret)
    }
    pub fn post_chat(&mut self, recipient: Faction, text: &str) -> Result<hyper::client::response::Response> {
        let client = try!(self.login());
        self.do_with_cookies(client.post("https://hvz.gatech.edu/chat/_post.php").body(&url::form_urlencoded::Serializer::new(String::new()).append_pair("aud", &format!("{:?}", recipient)).append_pair("content", text).finish()).header(form_type()), false).map_err(From::from)
    }
}

