use reqwest;
//use multipart;
use url;
use time;
use std;
//use std::io::Read;
use serde_json;
use serde::{/*Serialize,*/Deserialize};
use serde_json::{Value};
lazy_static!{
    static ref API_KEY: String = std::env::var("GROUPME_API_KEY").expect("GroupMe API key not supplied in environment.");
}
static API_URL: &'static str = "https://api.groupme.com/v3";
static IMAGE_API_URL: &'static str = "https://image.groupme.com";
//static API_KEY: &'static str = "hunter2";
use errors::*;

use super::attachments::Attachment;

macro_rules! client {
    () => (reqwest::Client::new()?);
    //() => (reqwest::Client::with_http_proxy("localhost", 8080));
}

#[inline] fn json_type() -> reqwest::header::ContentType { reqwest::header::ContentType::json() }

fn _empty_response(r: reqwest::Result<reqwest::Response>) -> Result<reqwest::Response> { Ok(r?.error_for_status().map_err(|e| -> Error { if e.is_client_error() || e.is_server_error() { if let Some(code) = e.status() { ErrorKind::HttpError(code).into() } else { e.into() } } else { e.into() } })?) }

fn slurp<R: std::io::Read>(mut r: R) -> std::io::Result<String> { let mut buffer = Vec::<u8>::new(); r.read_to_end(&mut buffer).and(Ok(String::from_utf8_lossy(&buffer).to_string())) }
fn empty_response(r: reqwest::Result<reqwest::Response>) -> Result<()> {
    let s = slurp(_empty_response(r)?)?;
    if s.trim().len() > 0 { bail!(ErrorKind::NonEmptyResponse(s)) } else { Ok(()) }
}

#[inline] fn clamp<T: Ord>(value: T, lower: T, upper: T) -> T { std::cmp::max(std::cmp::min(value, upper), lower) }
fn response(r: reqwest::Result<reqwest::Response>, key: &'static str) -> Result<Value> {
    let j = serde_json::from_reader(&mut _empty_response(r)?)?;
    let mut o = match j { Value::Object(m) => m, _ => { bail!(ErrorKind::JsonTypeError("top-lvl is no object")); } };
    match o.remove("status") {
        Some(Value::Number(n)) => { if n.as_u64() == Some(200u64) || n.as_f64() == Some(200.0f64) {} else {
            let x = n.as_u64().map(|i| format!("{:.0}", i)).or(n.as_f64().map(|i| format!("{:.0}", i))).unwrap().parse().unwrap_or(599u16);
            bail!(ErrorKind::HttpError(reqwest::StatusCode::try_from(x).unwrap_or(reqwest::StatusCode::Unregistered(x))))
        } },
        Some(_) => bail!(ErrorKind::JsonTypeError("out-of-band status had wrong type")),
        None => {},
    }
    match o.remove(key) { Some(x) => Ok(x), _ => bail!(ErrorKind::JsonTypeError("no response")) }
} // key="response" -> key="payload" for Image API. It's short-bus-special like that.

fn null_response(r: reqwest::Result<reqwest::Response>, key: &'static str) -> Result<()> {
    let j = serde_json::from_reader(&mut _empty_response(r)?)?;
    let mut o = match j { Value::Object(m) => m, _ => { bail!(ErrorKind::JsonTypeError("top-lvl is no object")); } };
    match o.remove("status") {
        Some(Value::Number(n)) => { if n.as_u64() == Some(200u64) || n.as_f64() == Some(200.0f64) {} else {
            let x = n.as_u64().map(|i| format!("{:.0}", i)).or(n.as_f64().map(|i| format!("{:.0}", i))).unwrap().parse().unwrap_or(599u16);
            bail!(ErrorKind::HttpError(reqwest::StatusCode::try_from(x).unwrap_or(reqwest::StatusCode::Unregistered(x))))
        } },
        Some(_) => bail!(ErrorKind::JsonTypeError("out-of-band status had wrong type")),
        None => {},
    }
    match o.remove(key) { Some(Value::Null) | None => Ok(()), _ => bail!(ErrorKind::JsonTypeError("response given when none expected")) }
} // key="response" -> key="payload" for Image API. It's short-bus-special like that.

#[inline] fn url_extend<I>(mut u: url::Url, segments: I) -> url::Url where I: IntoIterator, I::Item: AsRef<str> { u.path_segments_mut().unwrap().extend(segments); u }
#[inline] fn url_keyify(mut u: url::Url) -> url::Url { u.query_pairs_mut().clear().append_pair("token", &API_KEY); u }

pub trait Endpoint {
    #[inline] fn base_url() -> url::Url;
    #[inline] fn build_url<I>(segments: I) -> url::Url where I: IntoIterator, I::Item: AsRef<str> { url_keyify(url_extend(Self::base_url(), segments)) }
}

#[derive(Serialize)] pub struct GroupsCreateReqEnvelope { pub name: String, pub description: Option<String>, pub image_url: Option<String>, pub share: Option<bool> }
#[derive(Serialize)] pub struct GroupsUpdateReqEnvelope { #[serde(skip_serializing_if = "Option::is_none")] pub name: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] pub description: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] pub image_url: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] pub share: Option<bool> }

pub struct Groups;
impl Endpoint for Groups { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["groups"]) } }
impl Groups {
    pub fn show(group_id: &str) -> Result<Value> { response(client!().get(Self::build_url(&[group_id]))?.send(), "response") }
    pub fn index(page: Option<usize>, per_page: Option<usize>, former: Option<bool>) -> Result<Value> {
        let (page, per_page, former) = (page.unwrap_or(1), clamp(per_page.unwrap_or(500), 1, 500), former.unwrap_or(false));
        let mut u = Self::build_url(if former {vec!["former"]} else {vec![]});
        u.query_pairs_mut().append_pair("page", &format!("{}", page)).append_pair("per_page", &format!("{}", per_page));
        response(client!().get(u.as_str())?.send(), "response")
    }
    pub fn create(params: &GroupsCreateReqEnvelope) -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        response(client!().post(u.as_str())?.body(serde_json::to_string(params)?).header(json_type()).send(), "response")
    }
    pub fn update(group_id: &str, params: &GroupsUpdateReqEnvelope) -> Result<Value> {
        let u = Self::build_url(vec![group_id, "update"]);
        response(client!().post(u.as_str())?.body(serde_json::to_string(params)?).header(json_type()).send(), "response")
    }
    pub fn destroy(group_id: &str) -> Result<()> {
        let u = Self::build_url(vec![group_id, "destroy"]);
        empty_response(client!().post(u.as_str())?.send())
    }
    pub fn change_owners(group_id: &str, owner_id: &str) -> Result<()> {
        // GroupMe. Seriously?! You had to add a single endpoint with its *entire semantics* being snowflake-special?! For shame.
        let u = Self::build_url(vec!["change_owners"]);
        let mut o = serde_json::Map::new();
        o.insert("group_id".to_owned(), Value::String(group_id.to_owned()));
        o.insert("owner_id".to_owned(), Value::String(owner_id.to_owned()));
        let r = client!().post(u.as_str())?.body(serde_json::to_string(&Value::Array(vec![Value::Object(o)]))?).header(json_type()).send();
        let j = serde_json::from_reader(&mut _empty_response(r)?)?;
        let mut o = match j { Value::Object(m) => m, _ => {
            bail!(ErrorKind::JsonTypeError("top-lvl is no object"));
        } };
        let mut a = match o.remove("results") { Some(Value::Array(x)) => x, _ => {
            bail!(ErrorKind::JsonTypeError("no response"));
        } };
        let mut o = match a.pop() { Some(Value::Object(x)) => x, _ => {
            bail!(ErrorKind::JsonTypeError("no response"));
        } };
        match (o.remove("owner_id"), o.remove("group_id"), o.remove("status")) {
            (Some(Value::String(ref x1)), Some(Value::String(ref x2)), Some(Value::String(ref x3))) if (x1.as_str(), x2.as_str(), x3.as_str()) == (owner_id, group_id, "200") => { Ok(()) },
            (Some(Value::String(ref x1)), Some(Value::String(ref x2)), Some(Value::String(ref x3))) if (x1.as_str(), x2.as_str(), x3.as_str()) == (owner_id, group_id, "403") => { Ok(()) },
            _ => bail!(ErrorKind::GroupOwnershipChangeFailed)
        }
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)] pub struct MemberId { pub user_id: String, pub nickname: String, }
#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)] struct _MemberIds { members: Vec<MemberId> }
pub struct Members;
impl Endpoint for Members { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["groups"]) } }
impl Members {
    pub fn add<I: IntoIterator>(group_id: &str, members: I) -> Result<Value> where MemberId: From<I::Item> {
        let u = Self::build_url(vec![group_id, "members", "add"]);
        //let mut o = Value::Object(std::collections::BTreeMap::new());
        //o.as_object_mut().unwrap().insert("members".to_string(), Value::Array(members.into_iter().map(|x| MemberId::from(x).to_json()).collect::<Vec<MemberId>>()));
        let o = _MemberIds { members: members.into_iter().map(|x| MemberId::from(x)).collect::<Vec<MemberId>>() };
        response(client!().post(u.as_str())?.body(serde_json::to_string(&o)?).header(json_type()).send(), "response")
    }
    //pub fn results(group_id: &str, result_id: &str) -> Result<Value> {
    //    let u = Self::build_url(vec![group_id, "members", "results", result_id]);
    //    response(client!().post(u.as_str()).send(), "response")
    //}
    pub fn remove(group_id: &str, membership_id: &str) -> Result<()> {
        let u = Self::build_url(vec![group_id, "members", membership_id, "remove"]);
        empty_response(client!().post(u.as_str())?.send())
    }
}

pub trait MessageEndpoint : Endpoint {
    fn create<S: std::borrow::Borrow<str>>(group_id: &str, text: S, attachments: Vec<Attachment>) -> Result<Value>;
}

pub trait ReadMessageEndpoint : MessageEndpoint {
    fn index(group_id: &str, which: &Option<MessageSelector>, limit: Option<usize>) -> Result<Value>;
    fn conversation_id(sub_id: &str) -> Result<String>;
}

#[derive(Serialize)] struct MessagesCreateParameters<'a> { source_guid: String, text: &'a str, attachments: Vec<Attachment> }
#[derive(Serialize)] struct MessagesCreateEnvelope<'a> { message: MessagesCreateParameters<'a> }

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] pub enum MessageSelector { Before(String), Since(String), After(String) }
pub struct Messages;
impl Endpoint for Messages { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["groups"]) } }
impl ReadMessageEndpoint for Messages {
    fn index(group_id: &str, which: &std::option::Option<MessageSelector>, limit: Option<usize>) -> Result<Value> {
        let limit = clamp(limit.unwrap_or(100), 1, 100);
        let mut u = Self::build_url(vec![group_id, "messages"]);
        {
            let mut m = u.query_pairs_mut();
            match which {
                &Some(MessageSelector::After(ref s)) => { m.append_pair("after_id", s); },
                &Some(MessageSelector::Before(ref s)) => { m.append_pair("before_id", s); },
                &Some(MessageSelector::Since(ref s)) => { m.append_pair("since_id", s); },
                &None => ()
            }
            m.append_pair("limit", &format!("{}", limit));
        }
        response(client!().get(u.as_str())?.send(), "response")
    }
    #[inline] fn conversation_id(sub_id: &str) -> Result<String> { Ok(sub_id.into()) }
}
impl MessageEndpoint for Messages {
    fn create<S: std::borrow::Borrow<str>>(group_id: &str, text: S, attachments: Vec<Attachment>) -> Result<Value> {
        let u = Self::build_url(vec![group_id, "messages"]);
        let envelope = MessagesCreateEnvelope { message: MessagesCreateParameters {
            source_guid: { let t = time::get_time(); format!("{}-{}", t.sec, t.nsec) },
            text: text.borrow(),
            attachments: attachments
        } };

        response(client!().post(u.as_str())?.body(serde_json::to_string(&envelope)?).header(json_type()).send(), "response")
    }
}


#[derive(Serialize)] struct DirectMessagesCreateParameters<'a> { source_guid: String, recipient_id: &'a str, text: &'a str, attachments: Vec<Attachment> }
#[derive(Serialize)] struct DirectMessagesCreateEnvelope<'a> { direct_message: DirectMessagesCreateParameters<'a> }

pub struct DirectMessages;
impl Endpoint for DirectMessages { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["direct_messages"]) } }
impl ReadMessageEndpoint for DirectMessages {
    fn index(other_user_id: &str, which: &Option<MessageSelector>, _: Option<usize>) -> Result<Value> {
        let mut u = Self::build_url(vec![other_user_id]);
        {
            let mut m = u.query_pairs_mut();
            match which {
                &Some(MessageSelector::After(ref s)) => { m.append_pair("after_id", s); },
                &Some(MessageSelector::Before(ref s)) => { m.append_pair("before_id", s); },
                &Some(MessageSelector::Since(ref s)) => { m.append_pair("since_id", s); },
                &None => ()
            }
        }
        response(client!().get(u.as_str())?.send(), "response")
    }
    fn conversation_id(sub_id: &str) -> Result<String> { Ok(User::get()?.user_id + "+" + sub_id) }
}
impl MessageEndpoint for DirectMessages {
    fn create<S: std::borrow::Borrow<str>>(recipient_id: &str, text: S, attachments: Vec<Attachment>) -> Result<Value> {
        let u = Self::build_url(vec![recipient_id]);
        let envelope = DirectMessagesCreateEnvelope { direct_message: DirectMessagesCreateParameters {
            source_guid: { let t = time::get_time(); format!("{}-{}", t.sec, t.nsec) },
            recipient_id: recipient_id,
            text: text.borrow(),
            attachments: attachments
        } };
        response(client!().post(u.as_str())?.body(serde_json::to_string(&envelope)?).header(json_type()).send(), "response")
    }
}

pub struct Likes;
impl Endpoint for Likes { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["messages"]) } }
impl Likes {
    pub fn create(conversation_id: &str, message_id: &str) -> Result<()> {
        let u = Self::build_url(vec![conversation_id, message_id, "like"]);
        null_response(client!().post(u.as_str())?.send(), "response")
    }
    pub fn destroy(conversation_id: &str, message_id: &str) -> Result<()> {
        let u = Self::build_url(vec![conversation_id, message_id, "unlike"]);
        null_response(client!().post(u.as_str())?.send(), "response")
    }
}

#[derive(Serialize)] pub struct BotsCreateReqEnvelope { pub group_id: String, pub name: String, pub avatar_url: Option<String>, pub callback_url: Option<String> }
#[derive(Serialize)] struct BotsCreateEnvelope { bot: BotsCreateReqEnvelope }

#[derive(Serialize)] struct BotsMessageCreateEnvelope<'a> { bot_id: &'a str, text: &'a str, picture_url: Option<String>, attachments: Vec<Attachment> }

pub struct Bots;
impl Endpoint for Bots { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["bots"]) } }
impl MessageEndpoint for Bots {
    fn create<S: std::borrow::Borrow<str>>(bot_id: &str, text: S, attachments: Vec<Attachment>) -> Result<Value> {
        let u = Self::build_url(vec!["post"]);
        let envelope = BotsMessageCreateEnvelope {
            bot_id: bot_id,
            text: text.borrow(),
            picture_url: None,
            attachments: attachments
        };
        empty_response(client!().post(u.as_str())?.body(serde_json::to_string(&envelope)?).header(json_type()).send()).map(|()| Value::Null)
    }
}
impl Bots {
    pub fn index() -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        response(client!().get(u.as_str())?.send(), "response")
    }
    pub fn create(mut params: BotsCreateReqEnvelope) -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        if params.callback_url.is_none() {
            let mut example_com = url::Url::parse("http://example.com").unwrap();
            example_com.set_fragment(Some(params.name.as_str()));
            example_com.set_query(Some(User::get()?.user_id.as_str()));
            std::mem::replace(&mut params.callback_url, Some(example_com.into_string()));
        }
        response(client!().post(u.as_str())?.body(serde_json::to_string(&BotsCreateEnvelope { bot: params })?).header(json_type()).send(), "response")
    }
    pub fn destroy(bot_id: &str) -> Result<Value> {
        let u = Self::build_url(vec!["destroy"]);
        let mut m = serde_json::Map::new();
        m.extend(vec![("bot_id".to_string(), bot_id.to_string().into())]);
        /*
        let mut o = Value::Object(std::collections::BTreeMap::new());
        {
            let ref mut o = o;
            let mut m = o.as_object_mut().unwrap();//.get_mut("message").unwrap().as_object_mut().unwrap();
            m.insert("bot_id".to_string(), Value::String(bot_id.to_string()));
        }*/
        response(client!().post(u.as_str())?.body(serde_json::to_string(&Value::Object(m))?).header(json_type()).send(), "response")
    }
}

pub struct Users;
impl Endpoint for Users { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["users"]) } }
impl Users {
    pub fn me() -> Result<Value> {
        let u = Self::build_url(vec!["me"]);
        response(client!().get(u.as_str())?.send(), "response")
    }
}

pub struct Images;
    impl Endpoint for Images { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(IMAGE_API_URL).unwrap(), &["pictures"]) } }
impl Images {
    pub fn create<R: Into<reqwest::Body>>(image: R) -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        response(client!().post(u)?.body(image).header(reqwest::header::ContentType::png()).send(), "payload")
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct User { pub user_id: String, pub created_at: u64, pub updated_at: u64, pub id: String, pub name: String, pub email: Option<String>, pub phone_number: Option<String>, pub image_url: Option<String>, pub sms: Option<bool> }
impl User {
    //#[inline] fn nickname(&self) -> &str { &self.name }
    pub fn get() -> Result<Self> {
        Ok(Self::deserialize(Users::me()?)?)
    }
}

