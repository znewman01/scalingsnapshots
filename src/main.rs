#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::fmt::Debug;
use std::io;

use clap::Parser;
use serde::Serialize;

use sssim::authenticator::ClientSnapshot;
use sssim::log::Entry;
use sssim::simulator::{ResourceUsage, Simulator};
use sssim::{authenticator, Authenticator};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {}

#[derive(Debug, Serialize)]
struct Event {
    entry: Entry,
    result: ResourceUsage,
}

fn run<S, A>(authenticator: A)
where
    S: ClientSnapshot + Default + Debug,
    A: Authenticator<S> + Debug,
{
    let mut simulator = Simulator::new(authenticator);

    for line in io::stdin().lines() {
        let result = serde_json::from_str(&line.expect("stdin failed"));
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

fn main() {
    let _args: Args = Args::parse();

    // TODO: should be able to provide a configuration here
    let authenticator = authenticator::Insecure::default();

    run(authenticator);
}

#[test]
fn test_pass() {}
