use std;
use serde::{Deserialize/*,Serialize*/};
use reqwest;
use regex;
use render;
mod api;
mod attachments;

use errors::*;

pub use self::api::{MessageSelector};
use self::api::{ReadMessageEndpoint,MessageEndpoint};

pub use self::attachments::Attachment;
pub use self::api::User;

//use self::api::*;

//fn trace<T: Debug>(x: T) -> T { println!("{:?}", x); x }

pub trait ConversationId<E: ReadMessageEndpoint> { fn conversation_id(&self, sub_id: &str) -> Result<String> { E::conversation_id(sub_id) } }

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Message { pub id: String, source_guid: String, pub created_at: u64, pub user_id: String, pub recipient_id: Option<String>, pub group_id: Option<String>, pub name: String, /*pub avatar_url: String,*/ pub text: Option<String>, pub system: Option<bool>, pub favorited_by: Vec<String> }
impl Message {
    fn conversation_id(&self) -> Result<String> {
        let no_id = || ErrorKind::JsonTypeError("message had un-ID'ed parent").into();
        self.recipient_id.as_ref().map(|ref s| s.as_str()).ok_or(no_id()).and_then(self::api::DirectMessages::conversation_id).or(self.group_id.clone().ok_or(no_id()))
    }
    pub fn like(&self) -> Result<()> { self::api::Likes::create(&self.conversation_id()?, &self.id) }
    pub fn unlike(&self) -> Result<()> { self::api::Likes::destroy(&self.conversation_id()?, &self.id) }
    pub fn text(&self) -> String { self.text.clone().unwrap_or_default() } // GroupMe. Did you really have to put '"text": null' instead of '"text": ""'? Really?! Look at how much work this makes me do!
}

#[derive(Deserialize)] struct MessagesEnvelope { messages: Vec<Message> }

pub trait BidirRecipient<E: ReadMessageEndpoint> : Recipient<E> + ConversationId<E> {
    fn messages(&mut self, which: &Option<MessageSelector>, limit: Option<usize>) -> Result<Vec<Message>> {
        let backward = match which { &Some(MessageSelector::After(_)) => false, _ => true };
        E::index(self.id(), &which, limit).and_then(|m|
            Ok(MessagesEnvelope::deserialize(m)?.messages)).or_else(|e| {
                if let Error(ErrorKind::HttpError(reqwest::StatusCode::NotModified), _) = e { Ok(vec![]) } else { Err(e) }
            }).map(|mut m| { if backward { m.reverse(); } m })
    }
    fn slurp_messages(&mut self, selector: Option<MessageSelector>) -> Result<Vec<Message>> {
        let mut ret = Vec::<Message>::new();
        let mut selector = selector;
        let mut message_buffer = self.messages(&selector, Some(100))?;
        while message_buffer.len() > 0 {
            let id = message_buffer.last().unwrap().id.clone();
            ret.extend(message_buffer);
            std::mem::replace(&mut selector, match selector {
                Some(MessageSelector::After(_))  => Some(MessageSelector::After(id)),
                Some(MessageSelector::Before(_)) => Some(MessageSelector::Before(id)),
                Some(MessageSelector::Since(_))  => Some(MessageSelector::Since(id)),
                None                             => Some(MessageSelector::Before(id)),
            });
            message_buffer = self.messages(&selector, Some(100))?;
        }
        match selector { Some(MessageSelector::After(_)) => {}, _ => {ret.reverse()} };
        Ok(ret)
    }
}

pub trait Recipient<E: MessageEndpoint> {
    fn id(&self) -> &str;
    fn post_without_fallback<S: std::borrow::Borrow<str>>(&self, text: S, attachments: Option<Vec<Attachment>>) -> Result<Message> {
        if text.borrow().len() >= 1000 { return Err(ErrorKind::TextTooLong(text.borrow().to_owned(), attachments).into()); }
        Ok(Message::deserialize(E::create(self.id(), text, attachments.unwrap_or_default())?)?)
    }
    fn post<S: std::borrow::Borrow<str>>(&self, text: S, attachments: Option<Vec<Attachment>>) -> Result<Message> {
        match self.post_without_fallback(text, attachments) {
            Err(Error(ErrorKind::TextTooLong(t, a), _)) => {
                let (prelude, payload) = if let Some(first) = t.lines().map(ToOwned::to_owned).next() {
                    if first.trim().len() > 0 && first.len() < 500 {
                        (first.to_owned(), t.splitn(1, "\n").nth(2).unwrap_or("").trim().to_owned())
                    } else { ("(Long message was converted into image.)".to_owned(), t) }
                } else { ("(Long message was converted into image.)".to_owned(), t) };
                let mut a = a.unwrap_or_default();
                a.push(Attachment::upload_image(render::render(payload)?)?);
                self.post_without_fallback(prelude.to_owned(), Some(a))
            },
            x => x,
        }
    }
    fn post_mentioning<'a, S: std::borrow::Borrow<str>, S2: std::borrow::Borrow<str>, I: IntoIterator<Item=S2>>(&self, text: S, uids: I, attachments: Option<Vec<Attachment>>) -> Result<Message> {
        let (mentions, i) = Attachment::make_mentions(uids);
        let mut a = attachments.unwrap_or_default();
        a.push(mentions);
        self.post(format!("{: <1$}", text.borrow(), i), Some(a))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Bot { pub bot_id: String, pub group_id: String, pub name: String, pub avatar_url: Option<String>, pub callback_url: Option<String> }
impl Recipient<self::api::Bots> for Bot { #[inline] fn id(&self) -> &str { &self.bot_id } }
#[derive(Deserialize)] struct BotEnvelope { bot: Bot }
impl Bot {
    pub fn create(group: &Group, name: String, avatar_url: Option<String>, callback_url: Option<String>) -> Result<Self> { println!("Creating!"); Ok(BotEnvelope::deserialize(self::api::Bots::create(self::api::BotsCreateReqEnvelope { group_id: group.group_id.clone(), name: name, avatar_url: avatar_url, callback_url: callback_url })?)?.bot) }
    pub fn upsert(group: &Group, name: String, avatar_url: Option<String>, callback_url: Option<String>) -> Result<Self> {
        /* match Self::list()?.into_iter().find(|b| b.group_id == group.group_id && b.name == name) {
            Some(x) => Ok(x),
            None => Self::create(group, name, avatar_url, callback_url)
        } */
        Self::list()?.into_iter().find(|b| b.group_id == group.group_id && b.name == name).map(Ok).unwrap_or_else(|| Self::create(group, name, avatar_url, callback_url))
    }
    pub fn list() -> Result<Vec<Self>> { Ok(Vec::<Self>::deserialize(self::api::Bots::index()?)?) }
    pub fn destroy(self) -> std::result::Result<(), (Self, Error)> { Ok(self::api::Bots::destroy(&self.bot_id).map(|_| ()).map_err(|e| (self, e))?) }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Member { pub id: String, pub user_id: String, pub nickname: String, pub muted: bool, pub image_url: Option<String>, pub autokicked: bool, pub app_installed: Option<bool> }
impl ConversationId<self::api::DirectMessages> for Member {}
impl Recipient<self::api::DirectMessages> for Member { #[inline] fn id(&self) -> &str { &self.id } }
impl BidirRecipient<self::api::DirectMessages> for Member {}

impl From<Member> for self::api::MemberId { fn from(m: Member) -> self::api::MemberId { self::api::MemberId { user_id: m.user_id, nickname: m.nickname } } }
impl Member {
    pub fn canonical_name_of(name: &str) -> String {
        lazy_static! {
            static ref CALLSIGN : regex::Regex = regex::Regex::new(r#" *"[^"]+?" *"#).unwrap();
            static ref DAVID : regex::Regex = regex::Regex::new("Oso Oso").unwrap();
        }
        let name = DAVID.replace(&name, "David Oso");
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

#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct GroupMessagesInfo { pub count: u64, pub last_message_id: String, pub last_message_created_at: u64 }

#[derive(Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Group { pub id: String, pub group_id: String, pub name: String, pub description: Option<String>, pub image_url: Option<String>, pub creator_user_id: String, pub created_at: u64, pub updated_at: u64, pub share_url: Option<String>, pub office_mode: bool, pub phone_number: String, pub members: Vec<Member>, pub messages: GroupMessagesInfo }
impl ConversationId<self::api::Messages> for Group {}
impl Recipient<self::api::Messages> for Group { #[inline] fn id(&self) -> &str { &self.id } }
impl BidirRecipient<self::api::Messages> for Group {}
impl Group {
    pub fn create(name: String, description: Option<String>, image_url: Option<String>, share: Option<bool>) -> Result<Self> { Ok(Self::deserialize(self::api::Groups::create(&self::api::GroupsCreateReqEnvelope { name: name, description: description, image_url: image_url, share: share })?)?) }
    pub fn list() -> Result<Vec<Self>> {
        let mut page = 1;
        let mut groups = Vec::<Self>::new();
        let j = self::api::Groups::index(Some(page), Some(500), None)?;
        let mut next_groups = Vec::<Self>::deserialize(j)?;
        while next_groups.len() > 0 {
            groups.extend(next_groups.into_iter());
            page += 1;
            next_groups = Vec::<Self>::deserialize(self::api::Groups::index(Some(page), Some(500), None)?)?;
        }
        Ok(groups)
    }
    pub fn destroy(self) -> std::result::Result<(), (Self, Error)> { Ok(self::api::Groups::destroy(&self.group_id).map(|_| ()).map_err(|e| (self, e))?) }
    pub fn change_owners(&mut self, new_owner: &Member) -> Result<()> { let r = self::api::Groups::change_owners(&self.group_id, &new_owner.user_id)?; self.refresh()?; Ok(r) }
    pub fn refresh(&mut self) -> Result<()> {
        let id = self.id.clone();
        std::mem::replace(self, Self::deserialize(self::api::Groups::show(id.as_str())?)?);
        Ok(())
    }
    pub fn get(id: &str) -> Result<Self> { Ok(Self::deserialize(self::api::Groups::show(id)?)?) }
    pub fn update(&mut self, name: Option<String>, description: Option<String>, image_url: Option<String>, share: Option<bool>) -> Result<()> {
        let new_self = Self::deserialize(self::api::Groups::update(&self.group_id, &self::api::GroupsUpdateReqEnvelope { name: name, description: description, image_url: image_url, share: share })?)?;
        std::mem::replace(self, new_self);
        Ok(())
    }
    pub fn add_mut<I: IntoIterator>(&mut self, members: I) -> Result<()> where self::api::MemberId: From<I::Item> { let r = self.add(members)?; self.refresh()?; Ok(r) }
    pub fn add<I: IntoIterator>(&self, members: I) -> Result<()> where self::api::MemberId: From<I::Item> { self::api::Members::add(&self.id, members).and(Ok(())) } // If GroupMe's "Members Results" ever gets unfscked, result-ids will actually mean something, and we'll change this so we actually return them. Come to think of it, idk why I impl'ed the results endpoint in the first place...
    pub fn remove(&self, member: Member) -> Result<()> { match self.members.iter().find(|m| m.user_id == member.user_id) { Some(m) => self::api::Members::remove(&self.id, &m.id).and(Ok(())), None => bail!(ErrorKind::GroupRemovalFailed(member.clone())) } }
    pub fn remove_mut(&mut self, member: Member) -> Result<()> { let r = self.remove(member)?; self.refresh()?; Ok(r) }
    pub fn member_uids<'a: 'b, 'b>(&'a self) -> Box<Iterator<Item=&'b str> + 'b> {
        Box::new(self.members.iter().map(|ref m| m.user_id.as_str()))
    }
    pub fn member_uids_except<'a: 'b, 'b>(&'a self, exception: &'b str) -> Box<Iterator<Item=&'b str> + 'b> {
        Box::new(self.member_uids().filter(move |&u| { u != exception }))
    }
}

