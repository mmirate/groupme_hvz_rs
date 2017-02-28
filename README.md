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

## Prerequisites

- A Georgia Tech SSO account. (If you're a current student or employee of Georgia Tech, you have this.)
- A [Heroku](https://heroku.com) account.
- An [Uptime Robot](https://uptimerobot.com) account.

## Dependencies

This program depends on the Rust-language compiler, toolkit, and many third-party libraries. However, one part of the software used to deploy the program to Heroku (namely, the "buildpack") downloads all of this onto storage space associated with your Heroku account, so you do not need to install any software on your own machine.

## How to Use

(Technical users: it is possible to run this on your own machine if it's a modern Unix-like, but you'll need to figure everything out.)

### Deploy

Click this button:

[![Deploy](https://www.herokucdn.com/deploy/button.png)](https://heroku.com/deploy)

... and fill out the form with the information needed to deploy a copy of this program onto Heroku. Once you've clicked the big purple "Deploy" button, grab a donut or something while the program compiles. Finally, after completion, click the "View" button for further instructions.

When the program first runs, it will probably create a new Group for you; do *not* add anyone else to it, as it is used for you to command and control your instance of the program.

You should leave the program up-and-running on Heroku as long as you remain in the game's initial faction, regardless of whether any other players are also running the program.

### Verify

To check that the program is functioning on a basic level, send the exact message, "`!heartbeat please`" to your CnC Group. This should cause the program to post a brief message, on your behalf, in your faction's Group, within about 15 seconds.

### Activate

If other operators of this program ... change factions ... then you may be called-upon to move your instance of the program out of its default "dormant" state. To do this, follow the direction this program posts into the CnC Group upon startup: send the exact message "`!wakeup`" to the CnC Group.

### In Case of Faction-Change

If, on the other hand, *you* change factions, it is imperative that, *before* your faction-change is registered, you either (a) move your instance of this program back into its "dormant" state or (b) leave your ex-faction's Group.

Failure to do so may cause information leakage from your new faction to your ex-faction.

## Limitations

- Polls Georgia Tech HvZ's website every few seconds. This is bad for both sides' network performance and even worse for clientside power consumption. The original website frontend itself behaves no differently, however.
- Error handling is primitive (`try!(x :: Result<T, Box<Error>>)` at best; `unwrap()` at worst). Most errors will print an error and halt the current poll or send; non-transient errors (e.g. network down) will additionally spam stderr with highly-cryptic messages and possibly abort the program; you should take care that the program is restarted upon aborting.

