extern crate hyper;
extern crate url;
extern crate time;
use std;
pub use std::fmt::{Display,Debug};
pub use std::io::Read;
pub use std::error::Error;
pub use self::hyper::client::Client;
use rustc_serialize;
pub use rustc_serialize::{Encodable,Decodable};
pub use rustc_serialize::json::{Json,ToJson};
static API_URL: &'static str = "https://api.groupme.com/v3";
//static IMAGE_API_URL: &'static str = "https://image.groupme.com"; // I don't happen to need the Image API here
static API_KEY: &'static str = "hunter2";
use ::error::*;

#[inline] fn json_type() -> hyper::header::ContentType { hyper::header::ContentType(hyper::mime::Mime(hyper::mime::TopLevel::Application, hyper::mime::SubLevel::Json,vec![(hyper::mime::Attr::Charset,hyper::mime::Value::Utf8)])) }
//#[inline] fn form_type() -> hyper::header::ContentType { hyper::header::ContentType(hyper::mime::Mime(hyper::mime::TopLevel::Application, hyper::mime::SubLevel::WwwFormUrlEncoded,vec![(hyper::mime::Attr::Charset,hyper::mime::Value::Utf8)])) }

#[derive(Debug)]
pub struct HttpError(pub hyper::status::StatusCode);
impl Display for HttpError { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{:?}", self) } }
impl Error for HttpError { fn description<'a>(&'a self) -> &'a str { self.0.canonical_reason().unwrap_or("HTTP error (unknown code)") } }

fn _empty_response(r: hyper::Result<hyper::client::response::Response>) -> ResultB<hyper::client::response::Response> { let res = try!(r); if res.status.is_success() { Ok(res) } else { Err(Box::new(HttpError(res.status))) } }

fn slurp<R: std::io::Read>(mut r: R) -> std::io::Result<String> { let mut buffer = Vec::<u8>::new(); r.read_to_end(&mut buffer).and(Ok(String::from_utf8_lossy(&buffer).to_string())) }
fn empty_response(r: hyper::Result<hyper::client::response::Response>) -> ResultB<()> { let s = try!(slurp(try!(_empty_response(r)))); if s.trim().len() > 0 { Err(Box::new(rustc_serialize::json::DecoderError::MissingFieldError(s))) } else { Ok(()) } }

#[inline] fn clamp<T: Ord>(value: T, lower: T, upper: T) -> T { std::cmp::max(std::cmp::min(value, upper), lower) }
fn response(r: hyper::Result<hyper::client::response::Response>) -> ResultB<Json> {
    let j = try!(Json::from_reader(&mut try!(_empty_response(r))));
    let mut o = match j { Json::Object(m) => m, _ => { return Err(Box::new(rustc_serialize::json::DecoderError::MissingFieldError("top-lvl is no object".to_string()))); } };
    match o.remove("response") { Some(x) => Ok(x), _ => Err(Box::new(rustc_serialize::json::DecoderError::MissingFieldError("no response".to_string()))) }
} // "response" -> "data" for Image API. It's short-bus-special like that.

#[inline] fn url_extend<I>(mut u: url::Url, segments: I) -> url::Url where I: IntoIterator, I::Item: AsRef<str> { u.path_segments_mut().unwrap().extend(segments); u }
#[inline] fn url_keyify(mut u: url::Url) -> url::Url { u.query_pairs_mut().clear().append_pair("token", API_KEY); u }

pub trait Endpoint {
    #[inline] fn base_url() -> url::Url;
    #[inline] fn build_url<I>(segments: I) -> url::Url where I: IntoIterator, I::Item: AsRef<str> { url_keyify(url_extend(Self::base_url(), segments)) }
}

#[derive(RustcEncodable)] pub struct GroupsCreateReqEnvelope { pub name: String, pub description: Option<String>, pub image_url: Option<String>, pub share: Option<bool> }
#[derive(RustcEncodable)] pub struct GroupsUpdateReqEnvelope { pub name: Option<String>, pub description: Option<String>, pub image_url: Option<String>, pub share: Option<bool> }

pub struct Groups;
impl Endpoint for Groups { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["groups"]) } }
impl Groups {
    pub fn show(group_id: &str) -> ResultB<Json> { response(Client::new().get(Self::build_url(&[group_id])).send()) }
    pub fn index(page: Option<usize>, per_page: Option<usize>, former: Option<bool>) -> ResultB<Json> {
        let (page, per_page, former) = (page.unwrap_or(1), clamp(per_page.unwrap_or(500), 1, 500), former.unwrap_or(false));
        let mut u = Self::build_url(if former {vec!["former"]} else {vec![]});
        u.query_pairs_mut().append_pair("page", &format!("{}", page)).append_pair("per_page", &format!("{}", per_page));
        let r = Client::new().get(u.as_str()).send();
        //println!("{:?}", r);
        response(r)
    }
    pub fn create(params: &GroupsCreateReqEnvelope) -> ResultB<Json> {
        let u = Self::build_url(Vec::<&str>::new());
        response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(params).unwrap()).header(json_type()).send())
    }
    //pub fn oldcreate(name: String, description: Option<String>, image_url: Option<String>, share: Option<bool>) -> ResultB<Json> {
    //    let u = Self::build_url(Vec::<&str>::new());
    //    let mut o = Json::Object(std::collections::BTreeMap::new());
    //    {
    //        let ref mut o = o;
    //        let mut m = o.as_object_mut().unwrap();
    //        m.insert("name".to_string(), Json::String(name));
    //        description.map(|s| m.insert("description".to_string(), Json::String(s)));
    //        image_url.map(|s| m.insert("image_url".to_string(), Json::String(s)));
    //        m.insert("share".to_string(), Json::Boolean(share.unwrap_or(true)));
    //    }
    //    response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&o).unwrap()).header(json_type()).send())
    //}
    pub fn update(group_id: &str, params: &GroupsUpdateReqEnvelope) -> ResultB<Json> {
        let u = Self::build_url(vec![group_id, "update"]);
        response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(params).unwrap()).header(json_type()).send())
    }
    //pub fn oldupdate(group_id: &str, name: Option<String>, description: Option<String>, image_url: Option<String>, share: Option<bool>) -> ResultB<Json> {
    //    let u = Self::build_url(vec![group_id, "update"]);
    //    let mut o = Json::Object(std::collections::BTreeMap::new());
    //    {
    //        let ref mut o = o;
    //        let mut m = o.as_object_mut().unwrap(); // YES THIS NEEDS TO BE MUTABLE FFS
    //        name.map(|s| m.insert("name".to_string(), Json::String(s)));
    //        description.map(|s| m.insert("description".to_string(), Json::String(s)));
    //        image_url.map(|s| m.insert("image_url".to_string(), Json::String(s)));
    //        share.map(|b| m.insert("share".to_string(), Json::Boolean(b)));
    //    }
    //    response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&o).unwrap()).header(json_type()).send())
    //}
    pub fn destroy(group_id: &str) -> ResultB<()> {
        let u = Self::build_url(vec![group_id, "destroy"]);
        empty_response(Client::new().post(u.as_str()).send())
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)] pub struct MemberId { pub user_id: String, pub nickname: String, }
#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)] struct _MemberIds { members: Vec<MemberId> }
pub struct Members;
impl Endpoint for Members { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["groups"]) } }
impl Members {
    pub fn add<I: IntoIterator>(group_id: &str, members: I) -> ResultB<Json> where MemberId: From<I::Item> {
        let u = Self::build_url(vec![group_id, "members", "add"]);
        //let mut o = Json::Object(std::collections::BTreeMap::new());
        //o.as_object_mut().unwrap().insert("members".to_string(), Json::Array(members.into_iter().map(|x| MemberId::from(x).to_json()).collect::<Vec<MemberId>>()));
        let o = _MemberIds { members: members.into_iter().map(|x| MemberId::from(x)).collect::<Vec<MemberId>>() };
        response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&o).unwrap()).header(json_type()).send())
    }
    pub fn results(group_id: &str, result_id: &str) -> ResultB<Json> {
        let u = Self::build_url(vec![group_id, "members", "results", result_id]);
        response(Client::new().post(u.as_str()).send())
    }
    pub fn remove(group_id: &str, membership_id: &str) -> ResultB<()> {
        let u = Self::build_url(vec![group_id, "members", membership_id, "remove"]);
        empty_response(Client::new().post(u.as_str()).send())
    }
}

pub trait MessageEndpoint : Endpoint {
    fn index(group_id: &str, which: &Option<MessageSelector>, limit: Option<usize>) -> ResultB<Json>;
    fn create(group_id: &str, text: String, attachments: Vec<Json>) -> ResultB<Json>;
    fn conversation_id(sub_id: String) -> ResultB<String>;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)] pub enum MessageSelector { Before(String), Since(String), After(String) }
pub struct Messages;
impl Endpoint for Messages { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["groups"]) } }
impl MessageEndpoint for Messages {
    fn index(group_id: &str, which: &std::option::Option<MessageSelector>, limit: Option<usize>) -> ResultB<Json> {
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
        response(Client::new().get(u.as_str()).send())
    }
    fn create(group_id: &str, text: String, attachments: Vec<Json>) -> ResultB<Json> {
        let u = Self::build_url(vec![group_id, "messages"]);

        let mut m = std::collections::BTreeMap::new();
        let t = time::get_time();
        m.insert("source_guid".to_string(), Json::String(format!("{}-{}", t.sec, t.nsec)));
        m.insert("text".to_string(), Json::String(text));
        m.insert("attachments".to_string(), Json::Array(attachments));
        let mut m_p = std::collections::BTreeMap::new();
        m_p.insert("message".to_string(), Json::Object(m));
        //let mut o = Json::Object(std::collections::BTreeMap::new());
        //o.as_object_mut().unwrap().insert("message".to_string(), Json::Object(std::collections::BTreeMap::new()));
        //{
        //    let ref mut o = o;
        //    let mut m = o.as_object_mut().unwrap().get_mut("message").unwrap().as_object_mut().unwrap();
        //    let t = time::get_time();
        //    m.insert("source_guid".to_string(), Json::String(format!("{}-{}", t.sec, t.nsec)));
        //    m.insert("text".to_string(), Json::String(text));
        //    m.insert("attachments".to_string(), Json::Array(attachments));
        //}
        response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&Json::Object(m_p)).unwrap()).header(json_type()).send())
    }
    #[inline] fn conversation_id(sub_id: String) -> ResultB<String> { Ok(sub_id) }
}

pub struct DirectMessages;
impl Endpoint for DirectMessages { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["direct_messages"]) } }
impl MessageEndpoint for DirectMessages {
    fn index(other_user_id: &str, which: &Option<MessageSelector>, limit: Option<usize>) -> ResultB<Json> {
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
        response(Client::new().get(u.as_str()).send())
    }
    fn create(recipient_id: &str, text: String, attachments: Vec<Json>) -> ResultB<Json> {
        let u = Self::build_url(vec![recipient_id]);
        let mut o = Json::Object(std::collections::BTreeMap::new());
        o.as_object_mut().unwrap().insert("direct_message".to_string(), Json::Object(std::collections::BTreeMap::new()));
        {
            let ref mut o = o;
            let mut m = o.as_object_mut().unwrap().get_mut("direct_message").unwrap().as_object_mut().unwrap();
            let t = time::get_time();
            m.insert("source_guid".to_string(), Json::String(format!("{}-{}", t.sec, t.nsec)));
            m.insert("recipient_id".to_string(), Json::String(recipient_id.to_string()));
            m.insert("text".to_string(), Json::String(text));
            m.insert("attachments".to_string(), Json::Array(attachments));
        }
        response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&o).unwrap()).header(json_type()).send())
    }
    fn conversation_id(sub_id: String) -> ResultB<String> { Ok(try!(User::get()).user_id + "+" + &sub_id) }
}

#[derive(Clone)] pub struct Mentions { pub data: Vec<(String, usize, usize)> }
impl std::convert::Into<Json> for Mentions {
    fn into(self) -> Json {
        let mut o = std::collections::BTreeMap::new();
        o.insert("type".to_string(), "mentions".to_json());
        let mut user_ids = vec![];
        let mut loci = vec![];
        for (user_id, start, len) in self.data.into_iter() {
            user_ids.push(user_id.to_json());
            loci.push((start, len).to_json());
        }
        o.insert("user_ids".to_string(), Json::Array(user_ids));
        o.insert("loci".to_string(), Json::Array(loci));
        //o.insert("user_ids".to_string(), self.data.keys().cloned().collect::<Vec<String>>().to_json());
        //o.insert("loci".to_string(), self.data.values().cloned().collect::<Vec<String>>().to_json());
        Json::Object(o)
    }
}

pub struct Likes;
impl Endpoint for Likes { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["messages"]) } }
impl Likes {
    pub fn create(conversation_id: &str, message_id: &str) -> ResultB<()> {
        let u = Self::build_url(vec![conversation_id, message_id, "like"]);
        empty_response(Client::new().post(u.as_str()).send())
    }
    pub fn destroy(conversation_id: &str, message_id: &str) -> ResultB<()> {
        let u = Self::build_url(vec![conversation_id, message_id, "unlike"]);
        empty_response(Client::new().post(u.as_str()).send())
    }
}

pub struct Bots;
impl Endpoint for Bots { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["bots"]) } }
impl Bots {
    pub fn index() -> ResultB<Json> {
        let u = Self::build_url(Vec::<&str>::new());
        response(Client::new().get(u.as_str()).send())
    }
    pub fn create(group_id: String, name: String, avatar_url: Option<String>, callback_url: Option<String>) -> ResultB<Json> {
        let u = Self::build_url(Vec::<&str>::new());
        let mut o = Json::Object(std::collections::BTreeMap::new());
        o.as_object_mut().unwrap().insert("bot".to_string(), Json::Object(std::collections::BTreeMap::new()));
        {
            let ref mut o = o;
            let mut m = o.as_object_mut().unwrap().get_mut("bot").unwrap().as_object_mut().unwrap();
            let mut example_com = url::Url::parse("http://example.com").unwrap();
            example_com.set_fragment(Some(name.as_str()));
            m.insert("name".to_string(), Json::String(name));
            m.insert("group_id".to_string(), Json::String(group_id));
            m.insert("callback_url".to_string(), Json::String(callback_url.unwrap_or(example_com.into_string())));
            avatar_url.map(|s| m.insert("avatar_url".to_string(), Json::String(s)));
            //callback_url.map(|s| m.insert("callback_url".to_string(), Json::String(s)));
        }
        response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&o).unwrap()).header(json_type()).send())
    }
    pub fn post(bot_id: &str, text: String, attachments: Vec<Json>) -> ResultB<()> {
        let u = Self::build_url(vec!["post"]);
        let mut o = Json::Object(std::collections::BTreeMap::new());
        //o.as_object_mut().unwrap().insert("message".to_string(), Json::Object(std::collections::BTreeMap::new()));
        {
            let ref mut o = o;
            let mut m = o.as_object_mut().unwrap();//.get_mut("message").unwrap().as_object_mut().unwrap();
            //let t = time::get_time();
            //m.insert("source_guid".to_string(), Json::String(format!("{}-{}", t.sec, t.nsec)));
            m.insert("bot_id".to_string(), Json::String(bot_id.to_string()));
            m.insert("text".to_string(), Json::String(text));
            m.insert("picture_url".to_string(), Json::Null);
            m.insert("attachments".to_string(), Json::Array(attachments));
        }
        empty_response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&o).unwrap()).header(json_type()).send())
    }
    pub fn destroy(bot_id: &str) -> ResultB<Json> {
        let u = Self::build_url(vec!["destroy"]);
        let mut o = Json::Object(std::collections::BTreeMap::new());
        //o.as_object_mut().unwrap().insert("message".to_string(), Json::Object(std::collections::BTreeMap::new()));
        {
            let ref mut o = o;
            let mut m = o.as_object_mut().unwrap();//.get_mut("message").unwrap().as_object_mut().unwrap();
            m.insert("bot_id".to_string(), Json::String(bot_id.to_string()));
        }
        response(Client::new().post(u.as_str()).body(&rustc_serialize::json::encode(&o).unwrap()).header(json_type()).send())
    }
}

pub struct Users;
impl Endpoint for Users { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["users"]) } }
impl Users {
    pub fn me() -> ResultB<Json> {
        let u = Self::build_url(vec!["me"]);
        response(Client::new().get(u.as_str()).send())
    }
}

/*pub struct Images;
  impl Endpoint for Images { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(IMAGE_API_URL).unwrap(), &["pictures"]) } }
  impl Images {
  pub fn create()
  }*/

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)]
pub struct User { pub user_id: String, pub created_at: u64, pub updated_at: u64, pub id: String, pub name: String, pub email: Option<String>, pub phone_number: Option<String>, pub image_url: Option<String>, pub sms: Option<bool> }
impl User {
    #[inline] fn nickname(&self) -> &str { &self.name }
    fn get() -> ResultB<Self> { Ok(try!(Self::decode(&mut rustc_serialize::json::Decoder::new(try!(Users::me()))))) }
}

