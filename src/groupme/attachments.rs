use reqwest;
//use multipart;
//use url;
//use time;
use std;
//use std::io::Read;
//use serde_json;
use serde::{/*Serialize,*/Deserialize};
//use serde_json::{Value};
use errors::*;

#[derive(Clone, Debug)] #[derive(Serialize, Deserialize)] #[serde(tag = "type", rename_all = "lowercase")]
pub enum Attachment {
    Mentions { user_ids: Vec<String>, loci: Vec<(usize, usize)> },
    Image { url: String },
    Location { lat: String, lng: String, name: String },
    Split { token: String },
    Emoji { placeholder: String, charmap: Vec<(usize, usize)> },
}
impl Attachment {
    pub fn make_mentions<S: std::borrow::Borrow<str>, I: IntoIterator<Item=S>>(uids: I) -> (Self, usize) {
        let (user_ids, loci) : (Vec<_>, _) = uids.into_iter().enumerate().map(|(start,user_id)| (user_id.borrow().to_owned(), (start, 1))).unzip();
        let i = user_ids.len();
        (Attachment::Mentions { user_ids: user_ids, loci: loci }, i)
    }
    pub fn upload_image<R: Into<reqwest::Body>>(image: R) -> Result<Attachment> {
        #[derive(Deserialize)] struct ImageUrlEnvelope { url: String }
        Ok(Attachment::Image { url: ImageUrlEnvelope::deserialize(super::api::Images::create(image)?)?.url })
    }
}
