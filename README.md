# Groupme ↔ GA Tech HvZ

## Background

A student organization at the Georgia Institute of Technology, also known as Georgia Tech, hosts a campuswide game of tag (of sorts) which is based on, and uses the name of, "Humans vs Zombies" or "HvZ" for short.

In order to comply with Georgia Tech IT regulations, Georgia Tech HvZ uses, to track the state of the game and provide certain player-facing services, one of Georgia Tech's own webservers with Georgia Tech HvZ's own custom-made web application.

This custom-made web application includes a "Chat" system. This chat system is, in some cases, the only way to quickly contact other players, and is generally the only way to blast out important information to every member of one's faction.

This "Chat" system, however, is implemented by polling the server for the complete contents of the chat log.

As such, the "Chat" system cannot detect when new messages have arrived.

As such, the "Chat" system cannot use HTML5's Notification API to asynchronously tell me when I actually need to pay attention to it.

As such, the "Chat" system consumes much more mental resources than it should. That is a problem.

The general solution to this problem involves a third-party system called "GroupMe". However, there is no inherent linkage between GroupMe and the HvZ website's "Chat" system. More importantly: even with GroupMe, other parts of the HvZ website must be paid-attention-to in order to receive various other information. The other parts of the website do no better than "Chat" on the aforementioned metrics.

This is my solution to that problem.

## Dependencies

- [Heroku](https://www.heroku.com) (free-tier account ... *should* work? I think?) or the ability to emulate it locally (including the Postgres database)
- [Rust](https://www.rust-lang.org) (stable branch)

## How to Use

Caveat lector: I have not yet verified the correctness/completeness of these instructions. "It works on my machine."

### 1. Georgia Tech SSO credentials

Near the top of the file `src/hvz/mod.rs`, there are storage locations where these credentials are compiled into the program. Substitute these credentials with your own.

### 2. GroupMe API key

Same as before, in `src/groupme/api.rs`.

### 3. GroupMe groups

In addition to the Group used by your faction, this program also uses a dedicated, personal Group in order to send/receive commands from you.

In the file `Procfile`, replace the letter-"X" by the Group ID of your personal command&control Group.

### 4. Heroku

Compile the program by running `cargo build`.

Copy the program (`target/debug/groupme_hvz_rs`) to the root of the repo, commit, and deploy to Heroku. Follow any other relevant instructions in Heroku's documentation.

### 5. Verify

To check that the program is running, type the exact message, "`!heartbeat please`" into your command&control Group. Within 15 seconds, the bot should post a brief message on your behalf in your faction's Group.

## Limitations

- Polls the server every few seconds. This is bad for both sides' network performance and even worse for clientside power consumption. The original website frontend itself is no different, however.
- Since it uses a non-POE event loop, it cannot interact with you by using POE's IRCd; this is half bug, half feature: it's a bug in that being able to use Irssi would yield a lot of nice things for free, but it's a feature in that being strictly limited to one user prevents anyone from using this in a manner which might violate Georgia Tech IT regulations against storing SSO credentials.
	- (Note, this program does *not* request or store the primary key for the Georgia Tech SSO database entry that corresponds to you. Storing other people's credentials in-memory on the other hand... where you could dump them to disk as easily as the logs... if that isn't against policy then I would be very, very surprised.)

## Bugs/Limitations

- Error handling is primitive (`try!(x :: Result<T, Box<Error>>)` at best; `unwrap()` at worst). Most errors will print an error and halt the current poll or send; non-transient errors (e.g. network down) will additionally spam Heroku's logs with highly-cryptic messages; and if you're emulating Heroku locally then you should take care that the program is restarted upon crashing.
- Credentials are all baked into the program. TODO: factor these out into command-line parameters (which can be specified in the `Procfile`).
- Obtaining the Group-IDs to give this program, is nigh-impossible without using the GroupMe API. TODO: automatically discover/upsert the command&control Group.
- This program lacks the feature of laying dormant until activated via the command&control channel; impact is that you must be at a full-fledged computer in order to activate this program in the wake of a previous program-operator's ... err, faction-switch.

