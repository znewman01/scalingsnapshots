#![cfg_attr(feature = "strict", deny(warnings))]

use clap::{
    crate_authors, crate_description, crate_license, crate_name, crate_version, AppSettings, Clap,
};

#[derive(Clap)]
#[clap(name = crate_name!(), author=crate_authors!(", "), version=crate_version!())]
#[clap(license=crate_license!(), about=crate_description!())]
#[clap(setting=AppSettings::ColoredHelp)]
struct Args {}

fn main() {
    let _args: Args = Args::parse();
    println!("Hello, world!");
}

#[test]
fn test_pass() {}
