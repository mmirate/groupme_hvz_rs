use chrono;
use cookie;
use reqwest;
use scraper;
use url;
use std;
use std::iter::FromIterator;
use std::collections::BTreeMap;
use chrono::{TimeZone,Datelike};
use errors::*;

pub type Killboard = BTreeMap<Faction, Vec<Player>>;
pub type Chatboard = BTreeMap<Faction, Vec<Message>>;
pub type Panelboard = BTreeMap<PanelKind, Vec<Panel>>;

fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> { v.sort(); v }

#[derive(Copy, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(Serialize, Deserialize)] pub enum Faction { General, Human, Zombie, Admin }
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
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { Faction::Human => "human", Faction::Zombie => "zombie", Faction::Admin => "admin", Faction::General => "all", }) }
}
impl std::fmt::Binary for Faction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { Faction::Human => "h", Faction::Zombie => "z", Faction::Admin => "a", Faction::General => "?" }) }
}
impl<'b> From<&'b str> for Faction { fn from(s: &'b str) -> Faction { match s.to_lowercase().as_str() {
    "human" | "hum" | "h" => Faction::Human, "zombie" | "zomb" | "z" => Faction::Zombie, "admin" => Faction::Admin, _ => Faction::General,
} } }

use std::cmp::Ordering;

#[derive(Clone, Debug, Hash)] #[derive(Serialize, Deserialize)] pub struct Player { pub gtname: String, pub playername: String, pub faction: Faction, pub kb_gtname: Option<String> }
impl PartialEq for Player { fn eq(&self, other: &Player) -> bool { (&self.gtname, &self.playername) == (&other.gtname, &other.playername) } }
impl Eq for Player {}
impl Ord for Player { fn cmp(&self, other: &Player) -> Ordering { (&self.playername, &self.gtname).cmp(&(&other.playername, &other.gtname)) } }
impl PartialOrd for Player { fn partial_cmp(&self, other: &Player) -> Option<Ordering> { Some(self.cmp(other)) } }
//impl Player { pub fn erase(&self) -> Player { Player { faction: Faction::General, .. self.clone() } } }
impl Player {
    pub fn from_document(doc: scraper::Html) -> Result<Self> {
        let (faction_selector, playername_selector) = (scraper::Selector::parse("div.page-header > h3").map_err(|()| Error::from(ErrorKind::CSS))?, scraper::Selector::parse("div.page-header > h1").map_err(|()| Error::from(ErrorKind::CSS))?);
        Ok(Player {
            gtname: String::new(),
            playername: doc.select(&playername_selector).next().ok_or(Error::from(ErrorKind::Scraper("doc->Player.playername")))?.inner_html().trim().to_string(),
            faction: Faction::from(doc.select(&faction_selector).next().ok_or(Error::from(ErrorKind::Scraper("doc->Player.faction")))?.inner_html().trim()),
            kb_gtname: None
        })
    }
    pub fn from_kb_link<'a>(link: scraper::ElementRef<'a>) -> Result<Self> {
        Ok(Player {
            gtname: url::Url::parse("https://hvz.gatech.edu/killboard")?.join(link.value().attr("href").ok_or(Error::from(ErrorKind::Scraper("kb->link->href:Player.gtname")))?)?.query_pairs().next().ok_or(Error::from(ErrorKind::Scraper("join(kb->link->href):Player.gtname")))?.1.to_string(),
            playername: link.text().collect::<Vec<_>>().concat().trim().to_owned(),
            faction: Faction::default(),
            kb_gtname: link.parent().and_then(|td| td.next_sibling()).and_then(|td| td.next_sibling()).and_then(scraper::ElementRef::wrap).map(|td| td.text().collect::<Vec<_>>().concat().trim().to_owned())
        })
    }
    pub fn from_chat_link<'a>(link: scraper::ElementRef<'a>) -> Result<Self> {
        Ok(Player {
            gtname: url::Url::parse("https://hvz.gatech.edu/chat")?.join(link.value().attr("href").ok_or(Error::from(ErrorKind::Scraper("kb->link->href:Player.gtname")))?)?.query_pairs().next().ok_or(Error::from(ErrorKind::Scraper("join(kb->link->href):Player.gtname")))?.1.to_string(),
            playername: link.text().collect::<Vec<_>>().concat().trim().to_owned(),
            faction: Faction::default(),
            kb_gtname: None
        })
    }
    pub fn kb_playername<'a>(&'a self, zeds: &'a Vec<Player>) -> Option<&'a str> {
        if let Some(kb_gtname) = self.kb_gtname.as_ref() {
            if kb_gtname.to_lowercase() == "the admins" { return Some(kb_gtname); }
            for zed in zeds.iter() {
                if zed.gtname == *kb_gtname { return Some(&zed.playername); }
            }
            None
        } else { None }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(Serialize, Deserialize)] pub struct Message { pub id: usize, pub timestamp: chrono::DateTime<chrono::Local>, pub sender: Player, pub receiver: Faction, pub text: String, }
impl Message { pub fn from_tr<'a>(tr: scraper::ElementRef<'a>) -> Result<Self> {
    let col_selector = scraper::Selector::parse("td").map_err(|()| Error::from(ErrorKind::CSS))?;
    let link_selector = scraper::Selector::parse("a[href*=\"gtname\"]").map_err(|()| Error::from(ErrorKind::CSS))?;
    let cols : Vec<scraper::ElementRef> = tr.select(&col_selector).collect();
    Ok(Message{
        id: tr.value().attr("id").ok_or(Error::from(ErrorKind::CSS))?.splitn(2, "chat_line_").nth(1).ok_or(Error::from(ErrorKind::CSS))?.parse::<usize>().map_err(|_| Error::from(ErrorKind::CSS))?,
        sender: Player::from_chat_link(cols[0].select(&link_selector).next().ok_or(Error::from(ErrorKind::Scraper("chat->tr->link:Player")))?)?,
        timestamp: chrono::Local.datetime_from_str(&format!("{}/{}", chrono::Local::today().year(), cols[1].inner_html().trim()), "%Y/%m/%d %H:%M")?,
        receiver: Faction::default(), text: cols[2].inner_html().trim().to_owned()})
} }

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(Serialize, Deserialize)] pub enum PanelKind { Announcement, Mission }
impl std::fmt::Display for PanelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { PanelKind::Announcement => "announcement", PanelKind::Mission => "mission", }) }
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(Serialize, Deserialize)] pub struct MissionWW { pub start: chrono::DateTime<chrono::Local>, pub end: chrono::DateTime<chrono::Local>, pub location: String }
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
        let isoweek = chrono::Local::today().iso_week();
        let mut partial = chrono::format::Parsed::new();
        partial.set_isoyear(isoweek.year() as i64)?;
        partial.set_isoweek(isoweek.week() as i64)?;
        chrono::format::parse(&mut partial, s, chrono::format::StrftimeItems::new("%a %H:%M"))?;
        partial.to_datetime_with_timezone(&chrono::Local).map_err(From::from)
    }
}
#[derive(Clone, Debug)] #[derive(Serialize, Deserialize)] pub struct Panel { pub kind: PanelKind, pub title: String, pub particulars: Option<MissionWW>, pub text: String }
impl PartialEq for Panel { fn eq(&self, other: &Panel) -> bool {
    if let (&Some(_), &Some(_)) = (&self.particulars, &other.particulars) { (&self.kind, &self.particulars) == (&other.kind, &other.particulars) } else { (&self.kind, &self.title) == (&other.kind, &other.title) }
} }
impl Eq for Panel {}
impl Ord for Panel { fn cmp(&self, other: &Panel) -> Ordering {
    if let (&Some(_), &Some(_)) = (&self.particulars, &other.particulars) { (&self.kind, &self.particulars).cmp(&(&other.kind, &other.particulars)) } else { (&self.kind, &self.title).cmp(&(&other.kind, &other.title)) }
} }
impl PartialOrd for Panel { fn partial_cmp(&self, other: &Panel) -> Option<Ordering> { Some(self.cmp(other)) } }
impl Panel { pub fn from_div<'a>(div: scraper::ElementRef<'a>) -> Result<Panel> {
    let title_selector = scraper::Selector::parse(".panel-title").map_err(|()| Error::from(ErrorKind::CSS))?;
    let body_selector = scraper::Selector::parse(".panel-body" ).map_err(|()| Error::from(ErrorKind::CSS))?;
    let partics_selector = scraper::Selector::parse("p.mission_particulars").map_err(|()| Error::from(ErrorKind::CSS))?;
    Ok(Panel {
        kind: PanelKind::Announcement,
        title: div.select(&title_selector).next().ok_or(Error::from(ErrorKind::Scraper("Panel.title")))?.text().collect::<Vec<_>>().concat().trim().to_owned(),
        particulars: div.select(&partics_selector).next().and_then(MissionWW::from_p),
        text: div.select(&body_selector).next().ok_or(Error::from(ErrorKind::Scraper("Panel.text")))?.text().collect::<Vec<_>>().concat().trim().to_owned()
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

trait CookieJarExt {
    fn read_cookies<'b>(&self, rb: &'b mut reqwest::RequestBuilder) -> &'b mut reqwest::RequestBuilder;
    fn write_cookies<'b>(&mut self, res: &'b mut reqwest::Response) -> &'b mut reqwest::Response;
}

impl CookieJarExt for cookie::CookieJar {
    fn read_cookies<'b>(&self, rb: &'b mut reqwest::RequestBuilder) -> &'b mut reqwest::RequestBuilder {
        let mut h = reqwest::header::Cookie::new();
        for c in self.iter() { h.set(c.name().to_owned(), c.value().to_owned()); }
        rb.header(h)
    }
    fn write_cookies<'b>(&mut self, res: &'b mut reqwest::Response) -> &'b mut reqwest::Response {
        for c in res.headers().get::<reqwest::header::SetCookie>().unwrap_or(&reqwest::header::SetCookie(vec![])).iter().filter_map(|x| cookie::Cookie::parse(x.clone()).ok()) { self.remove(c.clone()); self.add(c); }
        res
    }
}

trait RequestBuilderExt { fn send_with_cookies(&mut self, &mut cookie::CookieJar, canfail: bool) -> Result<reqwest::Response>; }
impl RequestBuilderExt for reqwest::RequestBuilder {
    fn send_with_cookies(&mut self, jar: &mut cookie::CookieJar, canfail: bool) -> Result<reqwest::Response> {
        jar.read_cookies(self).send().map(|mut res| { jar.write_cookies(&mut res); res }).map_err(Error::from).and_then(|mut res| {
            if 509u16 == u16::from(res.status()) { bail!(ErrorKind::BandwidthLimitExceeded) }
            if !canfail { res = res.error_for_status().map_err(|e| -> Error { if e.is_client_error() || e.is_server_error() { if let Some(code) = e.status() { ErrorKind::HttpError(code).into() } else { e.into() } } else { e.into() } })?; }
            Ok(res)
        })
    }
}

#[derive(Debug)] struct HvZCreds { username: String, password: String }
impl HvZCreds {
    fn _fill_login_form(&self, doc: scraper::Html) -> Result<(BTreeMap<String, String>, url::Url)> {
        let form_selector = scraper::Selector::parse("form").map_err(|()| Error::from(ErrorKind::CSS))?;
        let form_control_selector = scraper::Selector::parse("form input[name][value]").map_err(|()| Error::from(ErrorKind::CSS))?;
        let mut queryparams = BTreeMap::new();
        queryparams.insert("username".to_owned(), self.username.clone());
        queryparams.insert("password".to_owned(), self.password.clone());
        for e in doc.select(&form_control_selector) {
            if !(e.value().attr("type").map(|t| ["reset", "checkbox"].contains(&t)).unwrap_or(true))
                && !(["username", "password"].contains(e.value().attr("name").as_ref().unwrap_or(&""))) {
                queryparams.insert(
                    e.value().attr("name" ).ok_or(Error::from(ErrorKind::Scraper("login->form->element[name]" )))?.to_owned(),
                    e.value().attr("value").ok_or(Error::from(ErrorKind::Scraper("login->form->element[value]")))?.to_owned()
                );
            }
        }
        let u = url::Url::parse("https://login.gatech.edu/")?.join(doc.select(&form_selector).next().ok_or(Error::from(ErrorKind::Scraper("login->form")))?.value().attr("action").ok_or(Error::from(ErrorKind::Scraper("login->form[action]")))?)?;/*"https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules"*/
        Ok((queryparams, u))
    }
}

#[derive(Debug)] pub struct HvZScraper { cookiejar: cookie::CookieJar, last_login: std::time::Instant, creds: HvZCreds }

impl HvZScraper {
    pub fn new(username: String, password: String) -> HvZScraper { HvZScraper { cookiejar: cookie::CookieJar::new(), last_login: std::time::Instant::now() - std::time::Duration::from_secs(1200), creds: HvZCreds { username: username, password: password } } }

    fn _slurp<R: std::io::Read>(mut r: R) -> std::io::Result<String> { let mut buffer = Vec::<u8>::new(); r.read_to_end(&mut buffer).and_then(|_| Ok(String::from_utf8_lossy(&buffer).to_string())) }
    #[inline] fn slurp(res: reqwest::Response) -> Result<String> { Ok(Self::_slurp(res)?) }
    pub fn login(&mut self) -> Result<reqwest::Client> {
        fn redirect_url_(res: &reqwest::Response) -> Option<url::Url> {
            res.headers().get::<reqwest::header::Location>().and_then(|ref loc| res.url().join(loc).ok())
        }
        let client = reqwest::Client::new();
        if self.last_login.elapsed() < std::time::Duration::from_secs(1200) { return Ok(client); }
        println!("Cached login is old; refreshing session.");
        let cookiejar = &mut self.cookiejar;
        let res = client.get("https://hvz.gatech.edu/rules/"/*"https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f"*/).send_with_cookies(cookiejar, true)?;
        let client = reqwest::Client::builder().redirect(reqwest::RedirectPolicy::none()).build()?;
        if res.url().host_str().unwrap_or("") != "hvz.gatech.edu" {
            let login_page = if res.url().host_str().map(|h| h == "login.gatech.edu").unwrap_or(false) {
                Self::slurp(res)?
            } else {
                Self::slurp(client.get("https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f").send_with_cookies(cookiejar, false)?)?
            };
            let (params, u) = self.creds._fill_login_form(scraper::Html::parse_document(login_page.as_str()))?;
            let mut res = client.post(u.as_str()).form(&params).send_with_cookies(cookiejar, true)?;
            while res.url().host_str().unwrap_or("") != "hvz.gatech.edu" {
                if let Some(loc) = redirect_url_(&res) { if loc.host_str().unwrap_or("") == "hvz.gatech.edu" { break; } }
                let document = scraper::Html::parse_document(Self::slurp(
                    client.get("https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f").send_with_cookies(cookiejar, true)?
                )?.as_str());
                let (params, u) = self.creds._fill_login_form(document)?;
                res = client.post(u.as_str()).form(&params).send_with_cookies(cookiejar, true)?;
                ensure!(res.status().is_success() || res.status().is_redirection(), ErrorKind::GaTechCreds);
            }
            while let Some(loc) = redirect_url_(&res) {
                res = client.get(loc).send_with_cookies(cookiejar, true)?;
                ensure!(res.status().is_success() || res.status().is_redirection(), ErrorKind::GaTechCreds);
            }
        }
        if client.get("https://hvz.gatech.edu/rules/").send_with_cookies(cookiejar, true)?.url().host_str().unwrap_or("") != "hvz.gatech.edu" {
            bail!(ErrorKind::GaTechCreds);
        }
        let client = reqwest::Client::new(); // reset the RedirectPolicy to default
        self.last_login = std::time::Instant::now();
        Ok(client)
    }
    pub fn whois(&mut self, gtname: &str) -> Result<Player> {
        let client = self.login()?;
        Player::from_document(scraper::Html::parse_document(Self::slurp(
            client.get(&format!("https://hvz.gatech.edu/profile/?gtname={}", gtname)).send_with_cookies(&mut self.cookiejar, false)?
        )?.as_str()))
    }
    fn shrink_to_fit<T>(mut v: Vec<T>) -> Vec<T> { v.shrink_to_fit(); v }
    #[inline] pub fn whoami(&mut self) -> Result<Player> { let u = self.creds.username.clone(); self.whois(u.as_str()) }
    #[inline] fn trace<T: std::fmt::Debug>(x: T) -> T { /* println!("{:?}", x); */ x }
    pub fn fetch_killboard(&mut self) -> Result<Killboard> {
        let client = self.login()?;

        let doc = scraper::Html::parse_document(Self::trace(Self::slurp({
            let cookiejar = &mut self.cookiejar;
            let mut res = client.get("https://hvz.gatech.edu/killboard/").send_with_cookies(cookiejar, false)?;
            while !res.url().as_str().contains("killboard") {
                println!("Capstoneurs...");
                res = client.get("https://hvz.gatech.edu/killboard/").send_with_cookies(cookiejar, false)?;
            }
            res
        })?.as_str()));

        let mut ret = Killboard::new();
        for faction in Faction::killboards() {
            ret.remove(&faction);
            ret.insert(faction, sorted(doc.select(&scraper::Selector::parse(&Self::trace(format!("#{:b}killboard a[href*=\"gtname\"]", &faction))).map_err(|()| Error::from(ErrorKind::CSS))?).map(|link| { Ok(Player { faction: faction, .. Player::from_kb_link(link)? }) }).collect::<Result<Vec<_>>>()?));
        }
        Ok(ret)
    }
    pub fn fetch_chatboard(&mut self) -> Result<Chatboard> {
        let client = self.login()?;

        let row_selector = scraper::Selector::parse("tr.chat_line").map_err(|()| Error::from(ErrorKind::CSS))?;

        let mut ret = Chatboard::new();
        for faction in Faction::chats() {
            if faction != Faction::General && faction != self.whoami()?.faction { continue; }
            ret.remove(&faction);
            ret.insert(faction, Self::shrink_to_fit(scraper::Html::parse_fragment(Self::slurp(
                client.post("https://hvz.gatech.edu/chat/_update.php").form(&[("aud",format!("{:?}", faction))]).send_with_cookies(&mut self.cookiejar, false)?
            )?.as_str()).select(&row_selector).map(|tr| Ok(Message { receiver: faction, .. Message::from_tr(tr)? })).collect::<Result<Vec<_>>>()?));
        }
        Ok(ret)
    }
    pub fn fetch_panelboard(&mut self) -> Result<Panelboard> {
        let client = self.login()?;
        let mut ret = Panelboard::new();
        for kind in vec![/*PanelKind::Announcement, */PanelKind::Mission] {
            ret.remove(&kind);
            ret.insert(kind, Self::shrink_to_fit(scraper::Html::parse_document(Self::slurp(client.get(&format!("https://hvz.gatech.edu/{}s", kind)).send_with_cookies(&mut self.cookiejar, false)?)?.as_str()).select(&scraper::Selector::parse(&format!("div.panel.{}", kind)).map_err(|()| Error::from(ErrorKind::CSS))?
            ).map(|div| Ok(Panel { kind: kind, .. Panel::from_div(div)? })).collect::<Result<Vec<_>>>()?));
        }
        Ok(ret)
    }
    pub fn post_chat(&mut self, recipient: Faction, text: &str) -> Result<reqwest::Response> {
        let client = self.login()?;
        client.post("https://hvz.gatech.edu/chat/_post.php").form(&[("aud", format!("{:?}", recipient).as_str()), ("content", text)]).send_with_cookies(&mut self.cookiejar, false).map_err(From::from)
    }
}

