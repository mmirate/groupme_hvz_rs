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
- A [Rust](https://www.rust-lang.org) installation (stable branch)

## How to Use

### 1. GroupMe API key

Log into https://dev.groupme.com and click the "Access Token" button. A popup will appear, containing a long string of gibberish in a bolded typeface. Call this string `$GROUPME_API_KEY`.

### 2. GroupMe groups

In addition to the Group used by your faction, this program also uses a dedicated, personal Group in order to send/receive commands from you. Create such a Group, with only yourself in it.

Run `GROUPME_API_KEY=$GROUPME_API_KEY cargo run --bin find_group` in a terminal. It will output a list of all Groups you're in, and their Group IDs. Find these two particular Groups in this list, and call their Group IDs `$FACTION_GROUP_ID` and `$CNC_GROUP_ID`, respectively.

### 3. Georgia Tech SSO credentials

Find these credentials and call them `$GATECH_USERNAME` and `$GATECH_PASSWORD`.

### 4. Run

Export the database connection URL in `DATABASE_URL`, and run `GROUPME_API_KEY=$GROUPME_API_KEY cargo run --release -- $FACTION_GROUP_ID $CNC_GROUP_ID $GATECH_USERNAME $GATECH_PASSWORD` to run the program. It will block the terminal whilst running.

### 5. Verify

To check that the program is functioning on a basic level, type the exact message, "`!heartbeat please`" into your command&control Group. Within 15 seconds, the bot should post a brief message, on your behalf, in your faction's Group.

## Limitations

- Polls the server every few seconds. This is bad for both sides' network performance and even worse for clientside power consumption. The original website frontend itself is no different, however.
- Error handling is primitive (`try!(x :: Result<T, Box<Error>>)` at best; `unwrap()` at worst). Most errors will print an error and halt the current poll or send; non-transient errors (e.g. network down) will additionally spam stderr with highly-cryptic messages and possibly abort the program; you should take care that the program is restarted upon aborting.

