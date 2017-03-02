use openssl;
use postgres;
use rustc_serialize;
use users;
use std;
use std::fmt::Debug;
use std::iter::Iterator;
use std::collections::BTreeMap;
use rustc_serialize::{Encodable,Decodable};
use errors::*;

pub struct Comm<I: std::iter::Iterator<Item=T>, J: std::iter::Iterator<Item=T>, T: Clone + Eq + Ord> { l: std::iter::Peekable<I>, r: std::iter::Peekable<J>, }
impl<I: std::iter::Iterator<Item=T>, J: std::iter::Iterator<Item=T>, T: Clone + Ord> Comm<I, J, T> {
    pub fn new(left: I, right: J) -> Comm<I, J, T> {
        Comm { l: left.peekable(), r: right.peekable(), }
    }
}

impl<I: std::iter::Iterator<Item=T>, J: std::iter::Iterator<Item=T>, T: Clone + Eq + Ord> Iterator for Comm<I, J, T> {
    type Item = (std::cmp::Ordering, T);
    fn next(&mut self) -> Option<Self::Item> { // http://stackoverflow.com/a/32020190/6274013
        let which = match (self.l.peek(), self.r.peek()) {
            (Some(l), Some(r)) => Some(l.cmp(r)),
            (Some(_), None)    => Some(std::cmp::Ordering::Less),
            (None, Some(_))    => Some(std::cmp::Ordering::Greater),
            (None, None)       => None,
        };

        match which {
            Some(o @ std::cmp::Ordering::Equal)   => self.r.next().and(self.l.next()).map(|x| (o, x)),
            Some(o @ std::cmp::Ordering::Less)    => self.l.next().map(|x| (o, x)),
            Some(o @ std::cmp::Ordering::Greater) => self.r.next().map(|x| (o, x)),
            None                                  => None,
        }
    }
}

fn shrink_to_fit<T>(mut v: Vec<T>) -> Vec<T> { v.shrink_to_fit(); v }

//pub fn comm_algorithm_memoryintensive<T>(left: Vec<T>, right: Vec<T>) -> Vec<(std::cmp::Ordering, T)> where T: Clone + Eq + Ord {
//    let mut ret: Vec<(std::cmp::Ordering, T)> = Vec::with_capacity(left.capacity()+right.capacity());
//    let (mut l, mut r) = (left.iter().peekable(), right.iter().peekable());
//    while l.peek().is_some() && r.peek().is_some() {
//        let x = l.peek().unwrap().clone();
//        let y = r.peek().unwrap().clone();
//        match x.cmp(y) {
//            o @ std::cmp::Ordering::Equal   => { ret.push((o, l.next().and(r.next()).unwrap().clone())); },
//            o @ std::cmp::Ordering::Less    => { ret.push((o, l.next()              .unwrap().clone())); },
//            o @ std::cmp::Ordering::Greater => { ret.push((o, r.next()              .unwrap().clone())); },
//        }
//    }
//    for item in l { ret.push((std::cmp::Ordering::Less, item.clone())); }
//    for item in r { ret.push((std::cmp::Ordering::Greater, item.clone())); }
//    shrink_to_fit(ret)
//}

fn comm_list<T>(new: Vec<T>, old: &Vec<T>, heed_deletions: bool) -> (Vec<T>, Vec<T>, Vec<T>) where T: Clone + Eq + Ord {
    let (mut all, mut additions, mut deletions) : (Vec<T>, Vec<T>, Vec<T>) = (Vec::with_capacity(new.len()), vec![], vec![]);
    for (o, x) in Comm::new(new.into_iter(), old.iter().cloned()) {
        match o {
            std::cmp::Ordering::Equal => all.push(x),
            std::cmp::Ordering::Less => { additions.push(x.clone()); all.push(x) },
            std::cmp::Ordering::Greater => { deletions.push(x.clone()); if !heed_deletions { all.push(x) } },
        }
    }
    (shrink_to_fit(all), shrink_to_fit(additions), shrink_to_fit(deletions))
}

fn comm_map<K, T>(mut new: BTreeMap<K, Vec<T>>, old: &mut BTreeMap<K, Vec<T>>, heed_deletions: bool) -> (BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>) where T: Debug + Clone + Decodable + Encodable + Eq + Ord, K: Debug + Ord + Clone + Decodable + Encodable {
    let (mut all, mut additions, mut deletions) : (BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());

    for k in old.keys() { new.entry(k.clone()).or_insert(vec![]); }
    for k in new.keys() { old.entry(k.clone()).or_insert(vec![]); }

    println!("{:?} vs {:?}", new.keys().collect::<Vec<_>>(), old.keys().collect::<Vec<_>>());
    for ((key, new_value), (ko, old_value)) in new.into_iter().zip(old.iter()) {
        assert!(&key == ko);
        let (a, d, l) : (Vec<T>, Vec<T>, Vec<T>) = comm_list::<T>(new_value, old_value, heed_deletions);
        all.remove(&key); all.insert(key.clone(), a);
        additions.remove(&key); additions.insert(key, d);
        deletions.remove(&ko); deletions.insert(ko.clone(), l);
    }

    //let keys = new.keys().cloned().collect::<Vec<K>>();
    //assert!(keys == old.keys().cloned().collect::<Vec<K>>());
    //for k in keys {
    //    let (new_v, old_v) = (new.get(&k).cloned().unwrap(), old.get(&k).cloned().unwrap());
    //    let (a, d, l) : (Vec<T>, Vec<T>, Vec<T>) = comm_list::<T>(new_v, old_v, heed_deletions);
    //    all.remove(&k); all.insert(k.clone(), a); additions.remove(&k); additions.insert(k.clone(), d); deletions.remove(&k); deletions.insert(k, l);
    //}
    (all, additions, deletions)
}

pub fn setup() -> postgres::Connection {
    let conn = postgres::Connection::connect(std::env::var("DATABASE_URL").unwrap_or(format!("postgresql://{}@localhost", users::get_user_by_uid(users::get_current_uid()).unwrap().name())).as_str(), postgres::SslMode::Prefer(&openssl::ssl::SslContext::new(openssl::ssl::SslMethod::Sslv23).unwrap())).unwrap();
    conn.execute("CREATE TABLE IF NOT EXISTS blobs (key VARCHAR PRIMARY KEY, val TEXT)", &[]).unwrap();
    conn
}

pub fn read(conn: &postgres::Connection, k: &str) -> postgres::Result<String> {
    match try!(conn.query("SELECT val FROM blobs WHERE key = $1", &[&k])).iter().next() { Some(r) => Ok(r.get("val")), None => Ok(String::new()) }
}

pub fn detect(conn: &postgres::Connection, k: &str) -> postgres::Result<bool> {
    match try!(conn.query("SELECT val FROM blobs WHERE key = $1", &[&k])).iter().next() { Some(_) => Ok(true), None => Ok(false) }
}


pub fn write(conn: &postgres::Connection, k: &str, v: &str) -> postgres::Result<u64> {
    // Yes, this relies on no concurrency. Don't try this at home, kids.
    let trans = try!(conn.transaction());
    let updates = try!(trans.execute(if try!(detect(conn, k)) { "UPDATE blobs SET val = $2 WHERE key = $1" } else { "INSERT INTO blobs(key,val) VALUES($1,$2)" }, &[&k, &v]));
    try!(trans.commit());
    Ok(updates)
}

pub fn write_dammit(conn: &postgres::Connection, k: &str, v: &str) -> postgres::Result<u64> {
    println!("Pre-write: k = {:?}, v = {:?}", k, v);
    let i = try!(write(conn, k, v));
    if i != 1 {
        println!("i = {:?}\nk = {:?}\nv = {:?}", i, k, v);
        assert!(false);
    }
    Ok(i)
}

#[inline] pub fn writeback<T>(conn: &postgres::Connection, k: &str, v: T) -> Result<usize> where T: rustc_serialize::Encodable + rustc_serialize::Decodable + Default {
    Ok(try!(write(conn, k, &try!(rustc_serialize::json::encode(&v)))) as usize)
}

pub fn readout<T>(conn: &postgres::Connection, k: &str) -> T where T: rustc_serialize::Encodable + rustc_serialize::Decodable + Default {
    match read(conn, k) {
        Ok(s) => match rustc_serialize::json::decode(s.as_str()) { Ok(x) => x, Err(_) => T::default() },
        Err(_) => T::default(),
    }
    //read(conn, k).map(String::as_str).and_then(rustc_serialize::json::decode).unwrap_or_default()
}

pub fn update_list<T>(conn: &postgres::Connection, k: &str, new: Vec<T>, old: &Vec<T>, heed_deletions: bool) -> Result<(Vec<T>, Vec<T>, Vec<T>)> where T: Clone + Decodable + Encodable + Eq + Ord + Debug {
    let (all, additions, deletions) = comm_list(new, old, heed_deletions);
    if !(additions.is_empty() && deletions.is_empty()) {
        let i = try!(write(conn, k, &try!(rustc_serialize::json::encode(&all))));
        if i != 1 {
            println!("i = {:?}\nall = {:?}", i, all);
            assert!(false);
        }
    }
    Ok((all, additions, deletions))
}

pub fn update_map<K, T>(conn: &postgres::Connection, k: &str, new: BTreeMap<K, Vec<T>>, old: &mut BTreeMap<K, Vec<T>>, heed_deletions: bool) -> Result<(BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>)> where T: Clone + rustc_serialize::Decodable + rustc_serialize::Encodable + Eq + Ord + Debug, K: Ord + Clone + Decodable + Encodable + Debug {
    let (all, additions, deletions) = comm_map(new, old, heed_deletions);
    if !(additions.is_empty() && deletions.is_empty()) {
        let i = try!(write(conn, k, &try!(rustc_serialize::json::encode(&all))));
        if i != 1 {
            println!("i = {:?}\nall = {:?}", i, all);
            assert!(false);
        }
    }
    Ok((all, additions, deletions))
}

