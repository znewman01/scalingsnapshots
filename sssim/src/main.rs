#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use clap::Parser;
use serde::Serialize;

use sssim::authenticator::ClientSnapshot;
use sssim::log::Entry;
use sssim::simulator::{ResourceUsage, Simulator};
use sssim::{accumulator, authenticator, Authenticator};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the file containing the stream of repository upload/download events.
    #[clap(long)]
    events_path: String,
    /// Path to the file containing the initial package state.
    #[clap(long)]
    init_path: String,
    /// Path to a file containing the configurations for authenticators to run.
    #[clap(long)]
    authenticator_config_path: Option<String>,
    /// The directory that output JSON files should be written to.
    #[clap(long)]
    output_directory: Option<String>,
}

#[derive(Debug, Serialize)]
struct Event {
    entry: Entry,
    result: ResourceUsage,
}

fn run<S, A, X, Y, Z>(authenticator: A, events: X, init: Y, mut out: Z)
where
    S: ClientSnapshot + Default + Debug,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Debug,
    X: BufRead,
    Y: BufRead,
    Z: io::Write,
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
        writeln!(out, "{}", json).expect("writing to output stream");
    }
}

fn main() -> io::Result<()> {
    let args: Args = Args::parse();

    let authenticator_configs: Vec<String> = match args.authenticator_config_path {
        Some(path) => Vec::from_iter(
            BufReader::new(File::open(path)?)
                .lines()
                .map(|l| l.expect("Reading configuration file failed.")),
        ),
        None => vec![
            "insecure".to_string(),
            "hackage".to_string(),
            "mercury_diff".to_string(),
            "mercury_hash".to_string(),
            "mercury_hash_diff".to_string(),
            "merkle".to_string(),
            "rsa".to_string(),
            "rsa_cached".to_string(),
            "vanilla_tuf".to_string(),
        ],
    };
    let output_directory = args.output_directory.unwrap_or_else(|| ".".to_string());

    for authenticator_config in authenticator_configs.iter() {
        let events = BufReader::new(File::open(args.events_path.clone())?);
        let init = BufReader::new(File::open(args.init_path.clone())?);
        let filename = format!("{}.json", authenticator_config);
        let out = File::create(Path::new(&output_directory).join(filename))?;
        println!("authenticator: {}", authenticator_config);
        match authenticator_config.as_str() {
            "insecure" => run(authenticator::Insecure::default(), events, init, out),
            "hackage" => run(authenticator::Hackage::default(), events, init, out),
            "mercury_diff" => run(authenticator::MercuryDiff::default(), events, init, out),
            "mercury_hash" => run(authenticator::MercuryHash::default(), events, init, out),
            "mercury_hash_diff" => {
                run(authenticator::MercuryHashDiff::default(), events, init, out)
            }
            "merkle" => run(authenticator::Merkle::default(), events, init, out),
            "rsa" => run(
                authenticator::Accumulator::<accumulator::rsa::RsaAccumulator>::default(),
                events,
                init,
                out,
            ),
            "rsa_cached" => run(
                authenticator::Accumulator::<
                    accumulator::CachingAccumulator<accumulator::RsaAccumulator>,
                >::default(),
                events,
                init,
                out,
            ),
            "vanilla_tuf" => run(authenticator::VanillaTuf::default(), events, init, out),
            _ => panic!("not valid"),
        };
    }

    Ok(())
}

#[test]
fn test_pass() {}
