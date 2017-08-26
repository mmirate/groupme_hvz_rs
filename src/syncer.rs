use postgres;
use serde_json;
use std;
use std::fmt::Debug;
use std::iter::Iterator;
use std::collections::BTreeMap;
use serde::{Serialize,Deserialize};
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

fn comm_map<'a, K, T>(mut new: BTreeMap<K, Vec<T>>, old: &'a mut BTreeMap<K, Vec<T>>, heed_deletions: bool) -> (BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>) where T: Debug + Clone + for<'de> Deserialize<'de> + Serialize + Eq + Ord, K: Debug + Ord + Clone + for<'de> Deserialize<'de> + Serialize {
    let (mut all, mut additions, mut deletions) : (BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());

    for k in old.keys() { new.entry(k.clone()).or_insert(vec![]); }
    for k in new.keys() { old.entry(k.clone()).or_insert(vec![]); }

    //println!("{:?} vs {:?}", new.keys().collect::<Vec<_>>(), old.keys().collect::<Vec<_>>());
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

pub fn setup() -> Result<postgres::Connection> {
    let conn = postgres::Connection::connect(std::env::var("DATABASE_URL")?.as_str(), postgres::TlsMode::Prefer(&postgres::tls::native_tls::NativeTls::new().unwrap()))?;
    conn.execute("CREATE TABLE IF NOT EXISTS blobs (key VARCHAR PRIMARY KEY, val TEXT)", &[])?;
    Ok(conn)
}

pub fn read(conn: &postgres::Connection, k: &str) -> Result<String> {
    Ok(conn.query("SELECT val FROM blobs WHERE key = $1", &[&k])?.iter().next().map(|r| r.get("val")).unwrap_or_else(String::new))
}

pub fn detect(conn: &postgres::Connection, k: &str) -> Result<bool> {
    Ok(conn.query("SELECT val FROM blobs WHERE key = $1", &[&k])?.iter().next().is_some())
}

pub fn write(conn: &postgres::Connection, k: &str, v: &str) -> Result<u64> {
    // Yes, the correctness of this methodology relies on a lack of concurrency. Don't try this at home, kids.
    let trans = conn.transaction()?;
    let updates = trans.execute(if detect(conn, k)? { "UPDATE blobs SET val = $2 WHERE key = $1" } else { "INSERT INTO blobs(key,val) VALUES($1,$2)" }, &[&k, &v])?;
    trans.commit()?;
    ensure!(updates == 1, ErrorKind::DbWriteNopped(k.to_string()));
    Ok(updates)
}

#[inline] pub fn writeback<T>(conn: &postgres::Connection, k: &str, v: &T) -> Result<u64> where T: Serialize + for<'de> Deserialize<'de> + Default {
    write(conn, k, &serde_json::to_string(v)?)
}

pub fn readout<T>(conn: &postgres::Connection, k: &str) -> T where T: Serialize + for<'de> Deserialize<'de> + Default {
    match read(conn, k) {
        Ok(s) => serde_json::from_str(s.clone().as_str()).unwrap_or(T::default()),
        Err(_) => T::default(),
    }
}

pub fn update_list<T>(conn: &postgres::Connection, k: &str, new: Vec<T>, old: &Vec<T>, heed_deletions: bool) -> Result<(Vec<T>, Vec<T>, Vec<T>)> where T: Clone + for<'de> Deserialize<'de> + Serialize + Eq + Ord + Debug {
    let (all, additions, deletions) = comm_list(new, old, heed_deletions);
    if !(additions.is_empty() && deletions.is_empty()) {
        writeback(conn, k, &all)?;
    }
    Ok((all, additions, deletions))
}

pub fn update_map<K, T>(conn: &postgres::Connection, k: &str, new: BTreeMap<K, Vec<T>>, old: &mut BTreeMap<K, Vec<T>>, heed_deletions: bool) -> Result<(BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>, BTreeMap<K, Vec<T>>)> where T: Clone + for<'de> Deserialize<'de> + Serialize + Eq + Ord + Debug, K: Ord + Clone + for<'de> Deserialize<'de> + Serialize + Debug {
    let (all, additions, deletions) = comm_map(new, old, heed_deletions);
    if !(additions.is_empty() && deletions.is_empty()) {
        writeback(conn, k, &all)?;
    }
    Ok((all, additions, deletions))
}

