extern crate rustc_serialize;
extern crate clap;
extern crate groupme_hvz_rs;
use groupme_hvz_rs::*;

#[derive(RustcEncodable, RustcDecodable)]
struct MyData {
    x: i64,
    y: i64,
    dirns: Option<bool>
}

#[derive(RustcEncodable, RustcDecodable)]
enum MyEnum {
    Point2 { x: i64, y: i64, name: String },
    Url { url: String },
}

#[derive(RustcEncodable, RustcDecodable)]
struct MyOtherData {
    location: (i64, i64),
    name: String
}

fn trivial_main() {
    println!("Hello, world!");
    println!("{}", rustc_serialize::json::encode(&(MyData { x: 1, y: 2, dirns: None } ) ).unwrap());
    println!("{}", rustc_serialize::json::encode(&(MyData { x: 3, y: 4, dirns: Some(false) } ) ).unwrap());
    println!("{}", rustc_serialize::json::encode(&(MyData { x: 5, y: 6, dirns: Some(true) } ) ).unwrap());
    println!("{}", rustc_serialize::json::encode(&(MyEnum::Point2 { x: -1, y: -2, name: "bundocks".to_string() } ) ).unwrap());
    println!("{}", rustc_serialize::json::encode(&(MyEnum::Url { url: "http://localhost".to_string() } ) ).unwrap());
    println!("{}", rustc_serialize::json::encode(&(MyOtherData { location: (-1, -2), name: "bundocks".to_string() } ) ).unwrap());
    println!("{}", rustc_serialize::json::encode(&groupme::Bot::list().unwrap()).unwrap());
    //println!("{:?}", hvz_scraper::HvZScraper::new().fetch_killboard());
    //println!("{:?}", hvz_scraper::HvZScraper::new().fetch_chatboard());
    //println!("{:?}", hvz_scraper::HvZScraper::new().fetch_panelboard());
    //println!("{:?}", groupme::Bot::list().unwrap()[0].post("Testing?".to_string(), vec![]).unwrap());
}

//use std;
use std::io::Write;

fn unwrap<T, E: std::fmt::Debug>(x: Result<T, E>) -> T { x.unwrap() }

fn actual_main() -> ! {
    let matches = clap::App::new("HvZ/GroupMe interactor").version("0.0.1").author("Milo Mirate <mmirate@gatech.edu>")
        .arg(clap::Arg::with_name("FACTION_GROUPID")
             .required(true)
             .index(1))
        .arg(clap::Arg::with_name("CNC_GROUPID")
             .required(true)
             .index(2))
        .get_matches();
    let (factiongroup1, factiongroup2, cncgroup) = (
        unwrap(groupme::Group::get(matches.value_of("FACTION_GROUPID").unwrap())),
        unwrap(groupme::Group::get(matches.value_of("FACTION_GROUPID").unwrap())),
        unwrap(groupme::Group::get(matches.value_of("CNC_GROUPID").unwrap()))
    );
    let mut conduits : Vec<Box<periodic::Periodic>> = vec![
        Box::new(conduit_to_groupme::ConduitHvZToGroupme::new(factiongroup1, cncgroup)),
        Box::new(conduit_to_hvz::ConduitGroupmeToHvZ::new(factiongroup2))
    ];
    println!("Alive!");
    let mut i = 0;
    loop {
        for c in conduits.iter_mut() {
            match c.tick(i) { Ok(()) => {}, Err(e) => { std::io::stderr().write(format!("\x07FATAL ERROR: {}", e).as_bytes()).unwrap(); } };
        }
        i += 1;
        i %= 1<<15;
        println!("==== Sleeping now. ====");
        std::thread::sleep(std::time::Duration::new(5,0));
    }
}

fn main() {
    //trivial_main()
    actual_main()
}
