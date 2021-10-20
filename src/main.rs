#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::io;

use clap::{
    crate_authors, crate_description, crate_license, crate_name, crate_version, AppSettings, Clap,
};
use serde::Serialize;

use sssim::log::Entry;
use sssim::simulator::{ResourceUsage, Simulator};
use sssim::{authenticator, Authenticator};

#[derive(Clap)]
#[clap(name = crate_name!(), author=crate_authors!(", "), version=crate_version!())]
#[clap(license=crate_license!(), about=crate_description!())]
#[clap(setting=AppSettings::ColoredHelp)]
struct Args {}

#[derive(Debug, Serialize)]
struct Event {
    entry: Entry,
    result: ResourceUsage,
}

fn main() {
    let _args: Args = Args::parse();
    // TODO: should be able to provide a configuration here

    let authenticator: Box<dyn Authenticator> = Box::new(authenticator::Insecure::default());
    let simulator: Simulator<Box<dyn Authenticator>> = Simulator::new(authenticator); // TODO: needs initial repo state

    for line in io::stdin().lines() {
        let result = serde_json::from_str(&line.expect("stdin failed"));
        let entry: Entry = result.expect("bad log entry");
        let usage = simulator.process(entry.action());
        let event = Event {
            entry,
            result: usage,
        };
        let json = serde_json::to_string(&event).unwrap();
        println!("{}", json);
    }
}

#[test]
fn test_pass() {}
