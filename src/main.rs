#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::io;

use clap::{
    crate_authors, crate_description, crate_license, crate_name, crate_version, AppSettings, Clap,
};
use serde::Serialize;

use sssim::log::LogEntry;
use sssim::simulator::{ResourceUsage, Simulator};

#[derive(Clap)]
#[clap(name = crate_name!(), author=crate_authors!(", "), version=crate_version!())]
#[clap(license=crate_license!(), about=crate_description!())]
#[clap(setting=AppSettings::ColoredHelp)]
struct Args {}

#[derive(Debug, Serialize)]
struct Event {
    entry: LogEntry,
    result: ResourceUsage,
}

fn main() {
    let _args: Args = Args::parse();
    let simulator = Simulator::new(); // TODO: needs initial repo state

    for line in io::stdin().lines() {
        let result = serde_json::from_str(&line.expect("stdin failed"));
        let entry: LogEntry = result.expect("bad log entry");
        let usage = simulator.process(&entry);
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
