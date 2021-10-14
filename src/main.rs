#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::io;

use clap::{
    crate_authors, crate_description, crate_license, crate_name, crate_version, AppSettings, Clap,
};

use sssim::log::LogEntry;

#[derive(Clap)]
#[clap(name = crate_name!(), author=crate_authors!(", "), version=crate_version!())]
#[clap(license=crate_license!(), about=crate_description!())]
#[clap(setting=AppSettings::ColoredHelp)]
struct Args {}

fn main() {
    let _args: Args = Args::parse();

    for line in io::stdin().lines() {
        let result = serde_json::from_str(&line.expect("stdin failed"));
        let _entry: LogEntry = result.expect("bad log entry");
    }

    println!("Hello, world!");
}

#[test]
fn test_pass() {}
