use std;
use std::fmt::Debug;
use rustc_serialize;
use rustc_serialize::{Decodable/*,Encodable*/};
use hyper;
use regex;
use render;
mod api;

use errors::*;

pub use self::api::{MessageSelector,Mentions};
use self::api::{ReadMessageEndpoint,MessageEndpoint};
use rustc_serialize::json::Json;

pub use self::api::User;

//use self::api::*;

//fn trace<T: Debug>(x: T) -> T { println!("{:?}", x); x }

pub trait ConversationId<E: ReadMessageEndpoint> { fn conversation_id(&self, sub_id: String) -> Result<String> { E::conversation_id(sub_id) } }

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)]
pub struct Message { pub id: String, source_guid: String, pub created_at: u64, pub user_id: String, pub recipient_id: Option<String>, pub group_id: Option<String>, pub name: String, /*pub avatar_url: String,*/ pub text: Option<String>, pub system: Option<bool>, pub favorited_by: Vec<String> }
impl Message {
    fn conversation_id(&self) -> Result<String> {
        let no_id = ErrorKind::JsonDecoding(rustc_serialize::json::DecoderError::MissingFieldError("message had un-ID'ed parent".to_string()));
        match self.recipient_id {
            Some(ref i) => self::api::DirectMessages::conversation_id(i.to_string()),
            None => self.group_id.clone().ok_or(no_id.into())
        }
        //self.recipient_id.ok_or(no_id).map(self::api::DirectMessages::conversation_id).or(self.group_id.ok_or(no_id)).unwrap()
    }
    pub fn like(&self) -> Result<()> { self::api::Likes::create(&try!(self.conversation_id()), &self.id) }
    pub fn unlike(&self) -> Result<()> { self::api::Likes::destroy(&try!(self.conversation_id()), &self.id) }
    pub fn text(&self) -> String { self.text.clone().unwrap_or_default() } // GroupMe. Did you really have to put '"text": null' instead of '"text": ""'? Really?! Look at how much work this makes me do!
}

#[derive(RustcDecodable)] struct MessagesEnvelope { messages: Vec<Message> }

pub trait BidirRecipient<E: ReadMessageEndpoint> : Recipient<E> + ConversationId<E> {
    //fn decode_messages(v: Vec<Json>) -> Result<Vec<Message>> {
    //    let mut ret = Vec::with_capacity(v.len());
    //    for m in v.into_iter() { ret.push(try!(Message::decode(&mut rustc_serialize::json::Decoder::new(m)))); }
    //    Ok(ret)
    //}
    fn messages(&mut self, which: &Option<MessageSelector>, limit: Option<usize>) -> Result<Vec<Message>> {
        let backward = match which { &Some(MessageSelector::After(_)) => false, _ => true };
        E::index(self.id(), &which, limit).and_then(|m|
            Ok(try!(MessagesEnvelope::decode(&mut rustc_serialize::json::Decoder::new(m))).messages)).or_else(|e|
                {
                    if let Error(ErrorKind::HttpError(hyper::status::StatusCode::NotModified), _) = e { Ok(vec![]) } else { Err(e) }
                }
            ).map(|mut m| { if backward { m.reverse(); } m })
    }
    //fn slurp_messages_after(&mut self, after: &Message) -> Result<Vec<Message>> {
    //    let mut ret = Vec::<Message>::new();
    //    let mut after = after.id.clone();
    //    let mut message_buffer: Vec<Message>;
    //    loop {
    //        message_buffer = try!(self.messages(&Some(MessageSelector::After(after)), Some(100)));
    //        if message_buffer.len() == 0 { break; }
    //        after = message_buffer.last().unwrap().id.clone();
    //        ret.extend(message_buffer);
    //    }
    //    Ok(ret)
    //}
    fn slurp_messages(&mut self, selector: Option<MessageSelector>) -> Result<Vec<Message>> {
        let mut ret = Vec::<Message>::new();
        let mut selector = selector;
        let mut message_buffer = try!(self.messages(&selector, Some(100)));
        while message_buffer.len() > 0 {
            let id = message_buffer.last().unwrap().id.clone();
            let wrong_id = message_buffer.first().unwrap().id.clone();
            println!("id = {:?}, wrong_id = {:?}, message_buffer.len() = {:?}", id, wrong_id, message_buffer.len());
            ret.extend(message_buffer);
            std::mem::replace(&mut selector, match selector {
                Some(MessageSelector::After(_))  => Some(MessageSelector::After(id)),
                Some(MessageSelector::Before(_)) => Some(MessageSelector::Before(id)),
                Some(MessageSelector::Since(_))  => Some(MessageSelector::Since(id)),
                None                             => Some(MessageSelector::Before(id)),
            });
            message_buffer = try!(self.messages(&selector, Some(100)));
        }
        match selector { Some(MessageSelector::After(_)) => {}, _ => {ret.reverse()} };
        println!("messages = {:?}", ret.iter().cloned().take(10).collect::<Vec<Message>>());
        Ok(ret)
    }
    //fn slurp_all_messages(&mut self) -> Result<Vec<Message>> {
    //    let mut ret = Vec::<Message>::new();
    //    let mut message_buffer = try!(self.messages(&None, Some(100)));
    //    if message_buffer.len() == 0 { message_buffer.reverse(); return Ok(message_buffer); }
    //    let mut before = message_buffer.last().unwrap().id.clone();
    //    ret.extend(message_buffer);
    //    loop {
    //        message_buffer = try!(self.messages(&Some(MessageSelector::Before(before)), Some(100)));
    //        if message_buffer.len() == 0 { break; }
    //        before = message_buffer.last().unwrap().id.clone();
    //        ret.extend(message_buffer);
    //    }
    //    ret.reverse();
    //    Ok(ret)
    //}
}

pub trait Recipient<E: MessageEndpoint> {
    fn id(&self) -> &str;
    //fn message_count(&self) -> usize;
    fn post(&self, text: String, attachments: Option<Vec<Json>>) -> Result<Json> {
        if text.len() >= 1000 { return Err(ErrorKind::TextTooLong(text, attachments).into()); }
        E::create(self.id(), text, attachments.unwrap_or_default())
    }
    fn post_or_post_image(&self, text: String, attachments: Option<Vec<Json>>) -> Result<Json> {
        match self.post(text, attachments) {
            Err(Error(ErrorKind::TextTooLong(t, a), _)) => {
                let (prelude, payload) = if let Some(first) = t.lines().map(ToOwned::to_owned).next() {
                    if first.trim().len() > 0 && first.len() < 500 {
                        (first.to_owned(), t.splitn(1, "\n").nth(2).unwrap_or("").trim().to_owned())
                    } else { ("(Long message was converted into image.)".to_owned(), t) }
                } else { ("(Long message was converted into image.)".to_owned(), t) };
                let mut a = a.unwrap_or_default();
                a.push(try!(upload_image(&mut try!(render::render(payload)).as_slice())));
                self.post(prelude.to_owned(), Some(a))
            },
            e @ Err(_) => e,
            x @ Ok(_) => x,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)]
pub struct Bot { pub bot_id: String, pub group_id: String, pub name: String, pub avatar_url: Option<String>, pub callback_url: Option<String> }
impl Recipient<self::api::DirectMessages> for Bot { #[inline] fn id(&self) -> &str { &self.bot_id } }
#[derive(RustcDecodable)] struct BotEnvelope { bot: Bot }
impl Bot {
    pub fn create(group: &Group, name: String, avatar_url: Option<String>, callback_url: Option<String>) -> Result<Self> { println!("Creating!"); Ok(try!(BotEnvelope::decode(&mut rustc_serialize::json::Decoder::new(try!(self::api::Bots::create(self::api::BotsCreateReqEnvelope { group_id: group.group_id.clone(), name: name, avatar_url: avatar_url, callback_url: callback_url }))))).bot) }
    pub fn upsert(group: &Group, name: String, avatar_url: Option<String>, callback_url: Option<String>) -> Result<Self> {
        //println!("{:?} - {:?}", Self::list(), name);
        match try!(Self::list()).into_iter().find(|b| b.group_id == group.group_id && b.name == name) {
            Some(x) => Ok(x),
            None => Self::create(group, name, avatar_url, callback_url)
        }
    }
    pub fn list() -> Result<Vec<Self>> { Ok(try!(Vec::<Self>::decode(&mut rustc_serialize::json::Decoder::new(try!(self::api::Bots::index()))))) }
    //pub fn post(&self, text: String, attachments: Option<Vec<Json>>) -> Result<()> { if text.len() >= 1000 { return Err(ErrorKind::TextTooLong(text, attachments).into()); } self::api::Bots::post(&self.bot_id, text, attachments.unwrap_or_default()) }
    pub fn destroy(self) -> std::result::Result<(), (Self, Error)> { Ok(try!(self::api::Bots::destroy(&self.bot_id).map(|_| ()).map_err(|e| (self, e)))) }
}


#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)]
pub struct Member { pub id: String, pub user_id: String, pub nickname: String, pub muted: bool, pub image_url: Option<String>, pub autokicked: bool, pub app_installed: Option<bool> }
impl ConversationId<self::api::DirectMessages> for Member {}
impl Recipient<self::api::DirectMessages> for Member { #[inline] fn id(&self) -> &str { &self.id } }
impl BidirRecipient<self::api::DirectMessages> for Member {}

impl From<Member> for self::api::MemberId { fn from(m: Member) -> self::api::MemberId { self::api::MemberId { user_id: m.user_id, nickname: m.nickname } } }
impl Member {
    pub fn canonical_name_of(name: &str) -> String {
        lazy_static! {
            static ref CALLSIGN : regex::Regex = regex::Regex::new(r#" *"[^"]+?" *"#).unwrap();
            static ref MATT : regex::Regex = regex::Regex::new("🖕").unwrap();
        }
        let name = MATT.replace(&name, "Matthew Zilvetti");
        let name = CALLSIGN.replace(&name, " ");
        let mut words_it = name.split_whitespace();
        let mut words = vec![];
        if let Some(first) = words_it.next() { words.push(first); }
        if let Some(last) = words_it.last() { words.push(last); }
        words.join(" ")
    }
    pub fn canonical_name(&self) -> String {
        Self::canonical_name_of(&self.nickname)
    }
}

fn unwrap<T,E: Debug>(r: std::result::Result<T,E>) -> T { r.unwrap() }

#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)]
pub struct GroupMessagesInfo { pub count: u64, pub last_message_id: String, pub last_message_created_at: u64 }

#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RustcDecodable, RustcEncodable)]
pub struct Group { pub id: String, pub group_id: String, pub name: String, pub description: Option<String>, pub image_url: Option<String>, pub creator_user_id: String, pub created_at: u64, pub updated_at: u64, pub share_url: Option<String>, pub office_mode: bool, pub phone_number: String, pub members: Vec<Member>, pub messages: GroupMessagesInfo }
impl ConversationId<self::api::Messages> for Group {}
impl Recipient<self::api::Messages> for Group { #[inline] fn id(&self) -> &str { &self.id } }
impl BidirRecipient<self::api::Messages> for Group {}
impl Group {
    pub fn create(name: String, description: Option<String>, image_url: Option<String>, share: Option<bool>) -> Result<Self> { Ok(try!(Self::decode(&mut rustc_serialize::json::Decoder::new(try!(self::api::Groups::create(&self::api::GroupsCreateReqEnvelope { name: name, description: description, image_url: image_url, share: share })))))) }
    pub fn list() -> Result<Vec<Self>> {
        let mut page = 1;
        let mut groups = Vec::<Self>::new();
        let j = try!(self::api::Groups::index(Some(page), Some(500), None));
        let mut next_groups = unwrap(Vec::<Self>::decode(&mut rustc_serialize::json::Decoder::new(j)));
        while next_groups.len() > 0 {
            groups.extend(next_groups.into_iter());
            page += 1;
            next_groups = try!(Vec::<Self>::decode(&mut rustc_serialize::json::Decoder::new(try!(self::api::Groups::index(Some(page), Some(500), None)))));
        }
        Ok(groups)
    }
    pub fn destroy(self) -> std::result::Result<(), (Self, Error)> { Ok(try!(self::api::Groups::destroy(&self.group_id).map(|_| ()).map_err(|e| (self, e)))) }
    pub fn change_owners(&mut self, new_owner: &Member) -> Result<()> { let r = Ok(try!(self::api::Groups::change_owners(&self.group_id, &new_owner.user_id))); try!(self.refresh()); r }
    pub fn refresh(&mut self) -> Result<Self> { let id = self.id.clone(); Ok(std::mem::replace(self, try!(Self::decode(&mut rustc_serialize::json::Decoder::new(try!(self::api::Groups::show(id.as_str()))))))) }
    pub fn get(id: &str) -> Result<Self> { Ok(try!(Self::decode(&mut rustc_serialize::json::Decoder::new(try!(self::api::Groups::show(id)))))) }
    pub fn update(&mut self, name: Option<String>, description: Option<String>, image_url: Option<String>, share: Option<bool>) -> Result<Self> { try!(self::api::Groups::update(&self.group_id, &self::api::GroupsUpdateReqEnvelope { name: name, description: description, image_url: image_url, share: share })); self.refresh() }
    pub fn add_mut<I: IntoIterator>(&mut self, members: I) -> Result<()> where self::api::MemberId: From<I::Item> { let r = try!(self.add(members)); try!(self.refresh()); Ok(r) }
    pub fn add<I: IntoIterator>(&self, members: I) -> Result<()> where self::api::MemberId: From<I::Item> { self::api::Members::add(&self.id, members).map(|_| ()) } // If GroupMe's "Members Results" ever gets unfscked, result-ids will actually mean something, and we'll change this so we actually return them. Come to think of it, idk why I impl'ed the results endpoint in the first place...
    pub fn remove(&self, member: Member) -> Result<()> { match self.members.iter().find(|m| m.user_id == member.user_id) { Some(m) => self::api::Members::remove(&self.id, &m.id).map(|_| ()), None => Err(ErrorKind::HttpError(hyper::status::StatusCode::NotFound).into()) } }
    pub fn remove_mut(&mut self, member: Member) -> Result<()> { let r = try!(self.remove(member)); try!(self.refresh()); Ok(r) }
    pub fn mention_everyone(&self) -> Json {
        Mentions { data: self.members.iter().enumerate().map(|(i,m)| (m.user_id.clone(), i, 1)).collect() }.into()
    }
    pub fn mention_everyone_except(&self, sender_uid: &str) -> Json {
        Mentions { data: self.members.iter().filter(|m| m.user_id != sender_uid).enumerate().map(|(i,m)| (m.user_id.clone(), i, 1)).collect() }.into()
    }
    pub fn post_to_everyone(&self, text: String, attachments: Option<Vec<Json>>) -> Result<()> {
        let mut a = vec![self.mention_everyone()];
        if let Some(ref attachments) = attachments { a.extend(attachments.iter().cloned()); }
        self.post(format!("{: <1$}", text, self.members.len()), Some(a)).and(Ok(()))
    }
}

#[derive(RustcDecodable)] struct ImageUrlEnvelope { url: String }
pub fn upload_image<R: std::io::Read>(image: &mut R) -> Result<Json> {
    let mut o = std::collections::BTreeMap::new();
    o.insert("url".to_string(), Json::String(try!(ImageUrlEnvelope::decode(&mut rustc_serialize::json::Decoder::new(try!(self::api::Images::create(image))))).url));
    Ok(Json::Object(o))
}

