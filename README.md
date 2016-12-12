# GroupMe ↔ GA Tech HvZ

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

- A local Postgres database
- [Rust](https://www.rust-lang.org) (stable branch)

## How to Use

### 1. Georgia Tech SSO credentials

Near the top of the file `src/hvz/mod.rs`, there are storage locations where these credentials are compiled into the program. Substitute these credentials with your own.

### 2. GroupMe API key

Same as previous, in `src/groupme/api.rs`. The API key can be obtained by logging in at https://dev.groupme.com.

### 3. GroupMe groups

In addition to the Group used by your faction, this program also uses a dedicated, personal Group in order to send/receive commands from you. Create such a Group, with only yourself in it.

Using another GroupMe API client, obtain the Group IDs for these two groups.

### 4. Run

Export the database connection URL in `DATABASE_URL`, and run `cargo run --release -- $FACTION_GROUP_ID $CNC_GROUP_ID` to run the program. It will block the terminal whilst running.

### 5. Verify

To check that the program is functioning on a basic level, type the exact message, "`!heartbeat please`" into your command&control Group. Within 15 seconds, the bot should post a brief message on your behalf in your faction's Group.

## Limitations

- Polls the server every few seconds. This is bad for both sides' network performance and even worse for clientside power consumption. The original website frontend itself is no different, however.
- This program lacks the feature of laying dormant until activated via the command&control channel; impact is that you must be at a full-fledged computer in order to activate this program in the wake of a previous program-operator's ... err, faction-switch.
- Error handling is primitive (`try!(x :: Result<T, Box<Error>>)` at best; `unwrap()` at worst). Most errors will print an error and halt the current poll or send; non-transient errors (e.g. network down) will additionally spam stderr with highly-cryptic messages and possibly abort the program; you should take care that the program is restarted upon aborting.
- Credentials are all compiled into the executable. TODO: factor these out into command-line parameters (which can be specified in the `Procfile`).
- Obtaining the Group-IDs to give this program, is nigh-impossible without using the GroupMe API. TODO: automatically discover/upsert the command&control Group.

