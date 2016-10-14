extern crate chrono;
extern crate hyper;
extern crate scraper;
extern crate url;
use std;
use std::collections::BTreeMap;
use self::chrono::TimeZone;
use error::*;

static USERNAME: &'static str = "jschmoe3";
static PASSWORD: &'static str = "hunter2";

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
}
impl std::fmt::Display for Faction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { Faction::Human => "human", Faction::Zombie => "zombie", Faction::Admin => "admin", Faction::General => "general", }) }
}
impl std::fmt::Debug for Faction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { Faction::Human => "hum", Faction::Zombie => "zomb", Faction::Admin => "admin", Faction::General => "all", }) }
}
impl<'b> From<&'b str> for Faction { fn from(s: &'b str) -> Faction { match s.to_lowercase().as_str() {
    "human" | "hum" | "h" => Faction::Human, "zombie" | "zomb" | "z" => Faction::Zombie, "admin" => Faction::Admin, _ => Faction::General,
} } }

use std::cmp::Ordering;

#[derive(Clone, Debug, Hash)] #[derive(RustcEncodable, RustcDecodable)] pub struct Player { pub gtname: String, pub playername: String, pub faction: Faction, }
impl PartialEq for Player { fn eq(&self, other: &Player) -> bool { (&self.gtname, &self.playername) == (&other.gtname, &other.playername) } }
impl Eq for Player {}
impl Ord for Player { fn cmp(&self, other: &Player) -> Ordering { (&self.gtname, &self.playername).cmp(&(&other.gtname, &other.playername)) } }
impl PartialOrd for Player { fn partial_cmp(&self, other: &Player) -> Option<Ordering> { Some(self.cmp(other)) } }
impl Player { pub fn erase(&self) -> Player { Player { faction: Faction::General, .. self.clone() } } }
impl<'a> From<scraper::Html> for Player { fn from(doc: scraper::Html) -> Player {
    let (faction_selector, playername_selector) = (scraper::Selector::parse("div.page-header > h3").unwrap(), scraper::Selector::parse("div.page-header > h1").unwrap());
    Player{gtname: String::new(), playername: doc.select(&playername_selector).next().unwrap().inner_html().trim().to_string(), faction: Faction::from(doc.select(&faction_selector).next().unwrap().inner_html().trim()) }
} }
impl<'a> From<scraper::ElementRef<'a>> for Player { fn from(link: scraper::ElementRef<'a>) -> Player {
    //let (fname_selector, lname_selector) = (scraper::Selector::parse("span.first-name").unwrap(), scraper::Selector::parse("span.last-name").unwrap());
    Player{gtname: url::Url::parse("https://hvz.gatech.edu/killboard").unwrap().join(link.value().attr("href").unwrap()).unwrap().query_pairs().next().unwrap().1.to_string(), playername: link.text().collect::<Vec<_>>().concat()/*link.select(&fname_selector).next().unwrap().inner_html().to_string() + &link.select(&lname_selector).next().unwrap().inner_html()*/, faction: Faction::default() }
} }

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(RustcEncodable, RustcDecodable)] pub struct Message { pub timestamp: chrono::DateTime<chrono::Local>, pub sender: Player, pub receiver: Faction, pub text: String, }
impl<'a> From<scraper::ElementRef<'a>> for Message { fn from(tr: scraper::ElementRef<'a>) -> Message {
    let col_selector = scraper::Selector::parse("td").unwrap();
    let link_selector = scraper::Selector::parse("a[href*=\"gtname\"]").unwrap();
    let cols : Vec<scraper::ElementRef> = tr.select(&col_selector).collect();
    Message{sender: Player::from(cols[0].select(&link_selector).next().unwrap()), timestamp: chrono::Local.datetime_from_str(&("2016/".to_owned() + &cols[1].inner_html()), "%Y/%m/%d %H:%M").unwrap(), receiver: Faction::default(), text: cols[2].inner_html().to_owned()}
} }

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(RustcEncodable, RustcDecodable)] pub enum PanelKind { Announcement, Mission }
impl std::fmt::Display for PanelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", match *self { PanelKind::Announcement => "announcement", PanelKind::Mission => "mission", }) }
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] #[derive(RustcEncodable, RustcDecodable)] pub struct Panel { pub kind: PanelKind, pub title: String }
impl<'a> From<scraper::ElementRef<'a>> for Panel { fn from(div: scraper::ElementRef<'a>) -> Panel {
    let title_selector = scraper::Selector::parse(".panel-title").unwrap();
    let body_selector = scraper::Selector::parse(".panel-body").unwrap();
    Panel{kind: PanelKind::Announcement, title: div.select(&title_selector).next().unwrap().text().collect::<Vec<_>>().concat(), /*text: div.select(&body_selector).next().unwrap().text().collect::<Vec<_>>().concat()*/ }
} }

type MyCookieJar = BTreeMap<String, hyper::header::CookiePair>;

#[derive(Clone, Debug)] pub struct HvZScraper { cookiejar: MyCookieJar, last_login: std::time::Instant, }

fn unwrap<T,E: std::fmt::Debug>(r: Result<T,E>) -> T { r.unwrap() }

impl HvZScraper {
    pub fn new() -> HvZScraper { HvZScraper { cookiejar: MyCookieJar::new(), last_login: std::time::Instant::now() - std::time::Duration::from_secs(1200)/*, client: hyper::client::Client::new()*/ } }
    fn read_cookies<'b>(&self, rb: hyper::client::RequestBuilder<'b>) -> hyper::client::RequestBuilder<'b> {
        let h = hyper::header::Cookie(self.cookiejar.values().cloned().collect());
        //println!("Cookie: {:?}", h);
        rb.header(h)
    }
    fn write_cookies(&mut self, res: hyper::client::response::Response) -> hyper::client::response::Response {
        /*for j in res.headers.get::<hyper::header::Cookie>() {
          for c in j.0.iter() {
          self.cookiejar.remove(&c.name); self.cookiejar.insert(c.name.clone(), c.clone());
          }
          }
          res*/
        //println!("Set-Cookie: {:?}", res.headers.get::<hyper::header::SetCookie>());
        res.headers.get::<hyper::header::SetCookie>().map(|j| j.0.iter().map(|c : &hyper::header::CookiePair| { self.cookiejar.remove(&c.name); self.cookiejar.insert(c.name.clone(), c.clone()) }).last()); res
    }
    fn slurp<R: std::io::Read>(mut r: R) -> std::io::Result<String> { let mut buffer = Vec::<u8>::new(); r.read_to_end(&mut buffer).and_then(|_| Ok(String::from_utf8_lossy(&buffer).to_string())) }
    //fn succeed(res: hyper::client::response::Response) -> hyper::client::response::Response { assert!(res.status.is_success()); res }
    fn do_with_cookies<'b>(&mut self, rb: hyper::client::RequestBuilder<'b>, canfail: bool) -> hyper::Result<hyper::client::response::Response> {
        let r = self.read_cookies(rb).send();
        //println!("response: {:?}", r);
        r.map(|res| self.write_cookies(res)).and_then(|res| if res.status.is_success() || canfail { Ok(res) } else { println!("NON-SLURP FAILED: {:?}", res); Err(hyper::error::Error::Method) })
    }
    fn do_and_slurp_with_cookies<'b>(&mut self, rb: hyper::client::RequestBuilder<'b>, read: bool, canfail: bool) -> hyper::Result<String> {
        self.read_cookies(rb).send().map(|res| self.write_cookies(res)).and_then(|res| if res.status.is_success() || canfail { if read { Self::slurp(res).map_err(hyper::error::Error::Io) } else { Ok(String::new()) } } else { println!("SLURP FAILED: {:?}", res); Err(hyper::error::Error::Method) })
    }
    fn _redirect_url(res: &hyper::client::response::Response) -> Option<url::Url> {
        match res.headers.get::<hyper::header::Location>() { Some(&hyper::header::Location(ref loc)) => { res.url.join(loc).ok() }, _ => None }
    }
    fn _fill_login_form(doc: scraper::Html) -> (String, url::Url) {
        let form_selector = scraper::Selector::parse("form").unwrap();
        let form_control_selector = scraper::Selector::parse("form input[name][value]").unwrap();
        let mut querystring = url::form_urlencoded::Serializer::new(String::new());
        let querystring = querystring.append_pair("username", USERNAME).append_pair("password", PASSWORD);
        doc.select(&form_control_selector).filter(|e| e.value().attr("type").map(|t| t!="reset" && t!="checkbox").unwrap_or(false) && e.value().attr("name").unwrap_or("") != "username" && e.value().attr("name").unwrap_or("") != "password").map(|e| { querystring.append_pair(e.value().attr("name").unwrap(), e.value().attr("value").unwrap()); }).collect::<Vec<()>>();
        let u = url::Url::parse("https://login.gatech.edu/").unwrap().join(doc.select(&form_selector).next().unwrap().value().attr("action").unwrap()).unwrap();/*"https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules"*/
        (querystring.finish(), u)
    }
    fn _login(&mut self) -> hyper::error::Result<hyper::client::Client> {
        let mut client = hyper::client::Client::new();
        if self.last_login.elapsed() < std::time::Duration::from_secs(600) { return Ok(client); }
        let res = self.do_with_cookies(client.get("https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f"), true);
        client.set_redirect_policy(hyper::client::RedirectPolicy::FollowNone);
        if res.is_err() || res.unwrap().url.host_str().unwrap() != "hvz.gatech.edu" {
            let (body, u) = Self::_fill_login_form(scraper::Html::parse_document(try!(self.do_and_slurp_with_cookies(client.get("https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f"), true, false)).as_str()));
            let mut res = try!(self.do_with_cookies(client.post(u.as_str()).body(&body).header(form_type()), true));
            while res.url.host_str().unwrap_or("") != "hvz.gatech.edu" {
                if let Some(loc) = Self::_redirect_url(&res) { if loc.host_str().unwrap_or("") == "hvz.gatech.edu" { break; } }
                let (body, u) = Self::_fill_login_form(scraper::Html::parse_document(try!(self.do_and_slurp_with_cookies(client.get("https://login.gatech.edu/cas/login?service=https%3a%2f%2fhvz.gatech.edu%2frules%2f"), true, true)).as_str()));
                res = try!(self.do_with_cookies(client.post(u.as_str()).body(&body).header(form_type()), true));
                if !(res.status.is_success() || res.status.is_redirection()) { return Err(hyper::error::Error::Method); }
            }
            while let Some(loc) = Self::_redirect_url(&res)  {
                res = try!(self.do_with_cookies(client.get(loc/*res.url.as_str()*/), true));
                if !(res.status.is_success() || res.status.is_redirection()) { return Err(hyper::error::Error::Method); }
            }
        }
        //println!("{:?}", &self.cookiejar);
        if !(try!(self.do_with_cookies(client.get("https://hvz.gatech.edu/rules/"), true)).url.host_str().unwrap_or("") == "hvz.gatech.edu") { // login credentials were correct?
            return Err(hyper::error::Error::Method);
        }
        client.set_redirect_policy(hyper::client::RedirectPolicy::FollowAll);
        self.last_login = std::time::Instant::now();
        Ok(client)
    }
    #[inline] pub fn login(&mut self) -> ResultB<hyper::client::Client> {
        Ok(try!(self._login()))
        //let /*mut*/ client = self._login();
        //let mut i = 0;
        //while client.is_err() && i < 10 {
        //client = self._login();
        //    i += 1;
        //}
        //client.unwrap()
    }
    pub fn whois(&mut self, gtname: &str) -> ResultB<Player> {
        let client = try!(self.login());
        Ok(Player::from(scraper::Html::parse_document(try!(self.do_and_slurp_with_cookies(client.get(&format!("https://hvz.gatech.edu/profile/?gtname={}", gtname)), true, false)).as_str())))
    }
    fn shrink_to_fit<T>(mut v: Vec<T>) -> Vec<T> { v.shrink_to_fit(); v }
    #[inline] pub fn whoami(&mut self) -> ResultB<Player> { self.whois(USERNAME) }
    #[inline] fn trace<T: std::fmt::Debug>(x: T) -> T { /* println!("{:?}", x); */ x }
    pub fn fetch_killboard(&mut self) -> ResultB<Killboard> {
        let client = try!(self.login());

        let mut ret = Killboard::new();
        for faction in Faction::killboards() {
            ret.remove(&faction);
            ret.insert(faction, Self::shrink_to_fit(sorted(scraper::Html::parse_document(try!(self.do_and_slurp_with_cookies(client.get("https://hvz.gatech.edu/killboard/"), true, false)).as_str()).select(&scraper::Selector::parse(&Self::trace(format!("#{}-killboard a[href*=\"gtname\"]", &faction))).unwrap()).map(|link| Player { faction: faction, .. Player::from(link) }).collect::<Vec<Player>>())));
        }
        Ok(ret)
    }
    pub fn fetch_chatboard(&mut self) -> ResultB<Chatboard> {
        let client = try!(self.login());

        let row_selector = scraper::Selector::parse("tr.chat_line").unwrap();
        //let col_selector = scraper::Selector::parse("td").unwrap();
        //let link_selector = scraper::Selector::parse("a[href*=\"gtname\"]").unwrap();

        let mut ret = Chatboard::new();
        for faction in Faction::chats() {
            if faction != Faction::General && faction != try!(self.whoami()).faction { continue; }
            ret.remove(&faction);
            ret.insert(faction, Self::shrink_to_fit(scraper::Html::parse_fragment(try!(self.do_and_slurp_with_cookies(client.post("https://hvz.gatech.edu/chat/_update.php").body(&format!("aud={:?}", faction)).header(form_type()), true, false)).as_str()).select(&row_selector).map(|x| Message { receiver: faction, .. Message::from(x) }).collect()));
        }
        Ok(ret)
    }
    pub fn fetch_panelboard(&mut self) -> ResultB<Panelboard> {
        let client = try!(self.login());
        let mut ret = Panelboard::new();
        for kind in vec![PanelKind::Announcement, PanelKind::Mission] {
            ret.remove(&kind);
            ret.insert(kind, Self::shrink_to_fit(scraper::Html::parse_document(try!(self.do_and_slurp_with_cookies(client.get(&format!("https://hvz.gatech.edu/{}s", kind)), true, false)).as_str()).select(&scraper::Selector::parse(&format!("div.panel.{}", kind)).unwrap()).map(|x| Panel { kind: kind, .. Panel::from(x) }).collect()));
        }
        Ok(ret)
    }
    pub fn post_chat(&mut self, recipient: Faction, text: &str) -> ResultB<hyper::client::response::Response> {
        let client = try!(self.login());
        Ok(try!(self.do_with_cookies(client.post("https://hvz.gatech.edu/chat/_post.php").body(&url::form_urlencoded::Serializer::new(String::new()).append_pair("aud", &format!("{:?}", recipient)).append_pair("content", text).finish()).header(form_type()), false)))
    }
}

