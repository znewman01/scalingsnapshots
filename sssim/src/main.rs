#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

use clap::Parser;
use serde::Serialize;

use sssim::authenticator::ClientSnapshot;
use sssim::log::Entry;
use sssim::simulator::{ResourceUsage, Simulator};
use sssim::{authenticator, Authenticator};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the file containing the stream of repository upload/download events.
    #[clap(long)]
    events_path: String,
    /// Path to the file containing the initial package state.
    #[clap(long)]
    init_path: String,
}

#[derive(Debug, Serialize)]
struct Event {
    entry: Entry,
    result: ResourceUsage,
}

fn run<S, A, X, Y>(authenticator: A, events: X, init: Y)
where
    S: ClientSnapshot + Default + Debug,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Debug,
    X: BufRead,
    Y: BufRead,
{
    let mut simulator = Simulator::new(authenticator);

    for line in init.lines() {
        let result = serde_json::from_str(&line.expect("reading from file failed"));
        let mut entry: Entry = result.expect("bad log entry");
        simulator.process(&mut entry.action); // ignore resource usage for initialization
    }

    for line in events.lines() {
        let result = serde_json::from_str(&line.expect("reading from file failed"));
        let mut entry: Entry = result.expect("bad log entry");
        let usage = simulator.process(&mut entry.action);
        let event = Event {
            entry,
            result: usage,
        };
        let json = serde_json::to_string(&event).unwrap();
        println!("{}", json);
    }
}

fn main() -> io::Result<()> {
    let args: Args = Args::parse();

    let events = File::open(args.events_path)?;
    let init = File::open(args.init_path)?;
    // TODO: should be able to provide a configuration here
    let authenticator = authenticator::Insecure::default();

    run(authenticator, BufReader::new(events), BufReader::new(init));

    Ok(())
}

#[test]
fn test_pass() {}
