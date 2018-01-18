use uuid;
use reqwest;
//use multipart;
use url;
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
    () => (reqwest::Client::new());
    //() => (reqwest::Client::with_http_proxy("localhost", 8080));
}

//#[inline] fn json_type() -> reqwest::header::ContentType { reqwest::header::ContentType::json() }

fn _empty_response(r: reqwest::Result<reqwest::Response>) -> Result<reqwest::Response> { Ok(r?.error_for_status().map_err(|e| -> Error { if e.is_client_error() || e.is_server_error() { if let Some(code) = e.status() { ErrorKind::HttpError(code).into() } else { e.into() } } else { e.into() } })?) }

fn slurp<R: std::io::Read>(mut r: R) -> std::io::Result<String> { let mut buffer = Vec::<u8>::new(); r.read_to_end(&mut buffer).and(Ok(String::from_utf8_lossy(&buffer).to_string())) }
fn empty_response(r: reqwest::Result<reqwest::Response>) -> Result<()> {
    let s = slurp(_empty_response(r)?)?;
    if s.trim().len() > 0 { bail!(ErrorKind::NonEmptyResponse(s)) } else { Ok(()) }
}

macro_rules! response_fn {
    ($name:ident => $key:ident : $keytype:ty) => {
        fn $name(r: reqwest::Result<reqwest::Response>) -> Result<$keytype> {
            #[derive(Deserialize, Debug)] struct GroupmeResponseEnvelope { status: u16, $key: $keytype, }
            let envelope : GroupmeResponseEnvelope = serde_json::from_reader(&mut _empty_response(r)?)?;
            if envelope.status != 200 {
                bail!(ErrorKind::HttpError(reqwest::StatusCode::try_from(envelope.status).unwrap_or(reqwest::StatusCode::Unregistered(envelope.status))))
            }
            Ok(envelope.$key)
        }
    };
    ($name:ident) => {
        fn $name(r: reqwest::Result<reqwest::Response>) -> Result<()> {
            #[derive(Deserialize, Debug)] struct GroupmeNullEnvelope { status: u16, }
            let envelope : GroupmeNullEnvelope = serde_json::from_reader(&mut _empty_response(r)?)?;
            if envelope.status != 200 {
                bail!(ErrorKind::HttpError(reqwest::StatusCode::try_from(envelope.status).unwrap_or(reqwest::StatusCode::Unregistered(envelope.status))))
            }
            Ok(())
        }
    };
}

#[inline] fn clamp<T: Ord>(value: T, lower: T, upper: T) -> T { std::cmp::max(std::cmp::min(value, upper), lower) }

response_fn!{response => response: Value}
response_fn!{image_response => payload: Value}
response_fn!{null_response}

#[inline] fn url_extend<I: IntoIterator>(mut u: url::Url, segments: I) -> url::Url where I::Item: AsRef<str> { u.path_segments_mut().unwrap().extend(segments); u }
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
    pub fn show(group_id: &str) -> Result<Value> { response(client!().get(Self::build_url(&[group_id])).send()) }
    pub fn index(page: Option<usize>, per_page: Option<usize>, former: Option<bool>) -> Result<Value> {
        let (page, per_page, former) = (page.unwrap_or(1), clamp(per_page.unwrap_or(500), 1, 500), former.unwrap_or(false));
        let mut u = Self::build_url(if former {vec!["former"]} else {vec![]});
        u.query_pairs_mut().append_pair("page", &format!("{}", page)).append_pair("per_page", &format!("{}", per_page));
        response(client!().get(u.as_str()).send())
    }
    pub fn create(params: &GroupsCreateReqEnvelope) -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        response(client!().post(u.as_str()).json(params).send())
    }
    pub fn update(group_id: &str, params: &GroupsUpdateReqEnvelope) -> Result<Value> {
        let u = Self::build_url(vec![group_id, "update"]);
        response(client!().post(u.as_str()).json(params).send())
    }
    pub fn destroy(group_id: &str) -> Result<()> {
        let u = Self::build_url(vec![group_id, "destroy"]);
        empty_response(client!().post(u.as_str()).send())
    }
    pub fn change_owners(group_id: &str, owner_id: &str) -> Result<()> {
        // GroupMe. Seriously?! You had to add a single endpoint with its *entire semantics* being snowflake-special?! For shame.
        #[derive(Deserialize, Debug)] struct GroupmeChangeOwnersResult { owner_id: String, group_id: String, status: /* UGH WHY?! */ String }
        #[derive(Deserialize, Debug)] struct GroupmeChangeOwnersResponseEnvelope { results: Vec<GroupmeChangeOwnersResult> }
        #[derive(Serialize, Debug)] struct GroupmeChangeOwnersReqEnvelope_<'a> { owner_id: &'a str, group_id: &'a str }
        let u = Self::build_url(vec!["change_owners"]);
        let r = client!().post(u.as_str()).json(&vec![GroupmeChangeOwnersReqEnvelope_ { owner_id: owner_id, group_id: group_id }]).send();
        let envelope: GroupmeChangeOwnersResponseEnvelope = serde_json::from_reader(&mut _empty_response(r)?)?;
        ensure!(envelope.results.into_iter().any(|e| (e.owner_id.as_str(), e.group_id.as_str()) == (owner_id, group_id) && (e.status == "200" || e.status == "403")), ErrorKind::GroupOwnershipChangeFailed);
        Ok(())
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Serialize)] pub struct MemberId { pub user_id: String, pub nickname: String, }
pub struct Members;
impl Endpoint for Members { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["groups"]) } }
impl Members {
    pub fn add<I: IntoIterator>(group_id: &str, members: I) -> Result<Value> where MemberId: From<I::Item> {
        let u = Self::build_url(vec![group_id, "members", "add"]);
        #[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Serialize)] struct _MemberIds { members: Vec<MemberId> }
        let o = _MemberIds { members: members.into_iter().map(|x| MemberId::from(x)).collect() };
        response(client!().post(u.as_str()).json(&o).send())
    }
    //pub fn results(group_id: &str, result_id: &str) -> Result<Value> {
    //    let u = Self::build_url(vec![group_id, "members", "results", result_id]);
    //    response(client!().post(u.as_str()).send())
    //}
    pub fn remove(group_id: &str, membership_id: &str) -> Result<()> {
        let u = Self::build_url(vec![group_id, "members", membership_id, "remove"]);
        empty_response(client!().post(u.as_str()).send())
    }
}

pub trait MessageEndpoint : Endpoint {
    fn create<S: std::borrow::Borrow<str>>(group_id: &str, text: S, attachments: Vec<Attachment>) -> Result<Value>;
}

pub trait ReadMessageEndpoint : MessageEndpoint {
    fn index(group_id: &str, which: &Option<MessageSelector>, limit: Option<usize>) -> Result<Value>;
    fn conversation_id(sub_id: &str) -> Result<String>;
}


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
        response(client!().get(u.as_str()).send())
    }
    #[inline] fn conversation_id(sub_id: &str) -> Result<String> { Ok(sub_id.into()) }
}
impl MessageEndpoint for Messages {
    fn create<S: std::borrow::Borrow<str>>(group_id: &str, text: S, attachments: Vec<Attachment>) -> Result<Value> {
        let u = Self::build_url(vec![group_id, "messages"]);
        #[derive(Serialize)] struct MessagesCreateParameters<'a> { source_guid: uuid::Uuid, text: &'a str, attachments: Vec<Attachment> }
        #[derive(Serialize)] struct MessagesCreateEnvelope<'a> { message: MessagesCreateParameters<'a> }
        let envelope = MessagesCreateEnvelope { message: MessagesCreateParameters {
            source_guid: uuid::Uuid::new_v4(),
            text: text.borrow(),
            attachments: attachments
        } };

        response(client!().post(u.as_str()).json(&envelope).send())
    }
}

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
        response(client!().get(u.as_str()).send())
    }
    fn conversation_id(sub_id: &str) -> Result<String> { Ok(User::get()?.user_id + "+" + sub_id) }
}
impl MessageEndpoint for DirectMessages {
    fn create<S: std::borrow::Borrow<str>>(recipient_id: &str, text: S, attachments: Vec<Attachment>) -> Result<Value> {
        let u = Self::build_url(vec![recipient_id]);
        #[derive(Serialize)] struct DirectMessagesCreateParameters<'a> { source_guid: uuid::Uuid, recipient_id: &'a str, text: &'a str, attachments: Vec<Attachment> }
        #[derive(Serialize)] struct DirectMessagesCreateEnvelope<'a> { direct_message: DirectMessagesCreateParameters<'a> }
        let envelope = DirectMessagesCreateEnvelope { direct_message: DirectMessagesCreateParameters {
            source_guid: uuid::Uuid::new_v4(),
            recipient_id: recipient_id,
            text: text.borrow(),
            attachments: attachments
        } };
        response(client!().post(u.as_str()).json(&envelope).send())
    }
}

pub struct Likes;
impl Endpoint for Likes { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["messages"]) } }
impl Likes {
    pub fn create(conversation_id: &str, message_id: &str) -> Result<()> {
        let u = Self::build_url(vec![conversation_id, message_id, "like"]);
        null_response(client!().post(u.as_str()).send())
    }
    pub fn destroy(conversation_id: &str, message_id: &str) -> Result<()> {
        let u = Self::build_url(vec![conversation_id, message_id, "unlike"]);
        null_response(client!().post(u.as_str()).send())
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
        empty_response(client!().post(u.as_str()).json(&envelope).send()).map(|()| Value::Null)
    }
}
impl Bots {
    pub fn index() -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        response(client!().get(u.as_str()).send())
    }
    pub fn create(mut params: BotsCreateReqEnvelope) -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        if params.callback_url.is_none() {
            let mut example_com = url::Url::parse("http://example.com").unwrap();
            example_com.set_fragment(Some(params.name.as_str()));
            example_com.set_query(Some(User::get()?.user_id.as_str()));
            std::mem::replace(&mut params.callback_url, Some(example_com.into_string()));
        }
        response(client!().post(u.as_str()).json(&BotsCreateEnvelope { bot: params }).send())
    }
    pub fn destroy(bot_id: &str) -> Result<Value> {
        let u = Self::build_url(vec!["destroy"]);
        #[derive(Serialize)] struct BotsDestroyReqEnvelope<'a> { bot_id: &'a str }
        response(client!().post(u.as_str()).json(&BotsDestroyReqEnvelope { bot_id: bot_id }).send())
    }
}

pub struct Users;
impl Endpoint for Users { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(API_URL).unwrap(), &["users"]) } }
impl Users {
    pub fn me() -> Result<Value> {
        let u = Self::build_url(vec!["me"]);
        response(client!().get(u.as_str()).send())
    }
}

pub struct Images;
    impl Endpoint for Images { #[inline] fn base_url() -> url::Url { url_extend(url::Url::parse(IMAGE_API_URL).unwrap(), &["pictures"]) } }
impl Images {
    pub fn create<R: Into<reqwest::Body>>(image: R) -> Result<Value> {
        let u = Self::build_url(Vec::<&str>::new());
        image_response(client!().post(u).body(image).header(reqwest::header::ContentType::png()).send())
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

