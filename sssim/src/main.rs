#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use clap::Parser;
use rusqlite::{Connection, DatabaseName};
use serde::Serialize;
use uom::si::information::byte;

use sssim::authenticator::ClientSnapshot;
use sssim::log::{Action, Entry};
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

const PROGRESS_BAR_INCREMENT: u64 = 100;
const DB_NAME: DatabaseName = DatabaseName::Main;

fn write_sqlite(event: Event, conn: &Connection) -> rusqlite::Result<usize> {
    let action = match event.entry.action {
        Action::RefreshMetadata { .. } => "refresh",
        Action::Download { .. } => "download",
        Action::Publish { .. } => "publish",
    };
    let user = match event.entry.action {
        Action::RefreshMetadata { user } => Some(user.0),
        Action::Download { user, .. } => Some(user.0),
        Action::Publish { .. } => None,
    };
    let server_compute_ns: u64 = event
        .result
        .server_compute
        .whole_nanoseconds()
        .try_into()
        .unwrap();
    let user_compute_ns: u64 = event
        .result
        .user_compute
        .whole_nanoseconds()
        .try_into()
        .unwrap();
    let mut statement = conn.prepare_cached(
        "
        INSERT INTO results (
             timestamp,
             action,
             user,
             server_compute_ns,
             user_compute_ns,
             bandwidth_bytes,
             server_storage_bytes
             ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7 )
    ",
    )?;
    statement.execute(rusqlite::params![
        event.entry.timestamp.unix_timestamp(),
        action,
        user,
        server_compute_ns,
        user_compute_ns,
        event.result.bandwidth.get::<byte>(),
        event.result.storage.get::<byte>(),
    ])
}

fn run<S, A, X, Y>(authenticator: A, events: X, init: Y, out: &Connection) -> rusqlite::Result<()>
where
    S: ClientSnapshot + Default + Debug,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Debug,
    X: BufRead,
    Y: BufRead,
{
    out.execute(
        "CREATE TABLE results (
                 id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                 timestamp            INTEGER NOT NULL,
                 action               TEXT NOT NULL,
                 user                 TEXT,
                 server_compute_ns    INTEGER,
                 user_compute_ns      INTEGER,
                 bandwidth_bytes      INTEGER,
                 server_storage_bytes INTEGER
                 )",
        [],
    )?;
    let mut simulator = Simulator::new(authenticator);

    let bar = indicatif::ProgressBar::new(362350);
    let mut count = 0;
    bar.set_message("Initializing");
    for line in init.lines() {
        count += 1;
        if count % PROGRESS_BAR_INCREMENT == 0 {
            bar.inc(PROGRESS_BAR_INCREMENT);
        }
        let result = serde_json::from_str(&line.expect("reading from file failed"));
        let mut entry: Entry = result.expect("bad log entry");
        simulator.process(&mut entry.action); // ignore resource usage for initialization
    }
    bar.finish();

    let bar = indicatif::ProgressBar::new(2269238);
    let mut count = 0;
    bar.set_message("Simulating");
    for line in events.lines() {
        count += 1;
        if count % PROGRESS_BAR_INCREMENT == 0 {
            bar.inc(PROGRESS_BAR_INCREMENT);
        }
        let result = serde_json::from_str(&line.expect("reading from file failed"));
        let mut entry: Entry = result.expect("bad log entry");
        if let Action::Download { user, .. } = entry.action.clone() {
            let refresh_metadata = Action::RefreshMetadata { user };
            let mut inner_entry: Entry = Entry::new(entry.timestamp, refresh_metadata);
            let refresh_usage = simulator.process(&mut inner_entry.action);
            let event = Event {
                entry: inner_entry,
                result: refresh_usage,
            };
            write_sqlite(event, out)?;
        }
        let usage = simulator.process(&mut entry.action);
        let event = Event {
            entry,
            result: usage,
        };
        write_sqlite(event, out)?;
    }
    bar.finish();
    Ok(())
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
        let out_db = Connection::open_in_memory().expect("creating SQLite db");
        println!("authenticator: {}", authenticator_config);
        match authenticator_config.as_str() {
            "insecure" => run(authenticator::Insecure::default(), events, init, &out_db).unwrap(),
            "hackage" => run(authenticator::Hackage::default(), events, init, &out_db).unwrap(),
            "mercury_diff" => {
                run(authenticator::MercuryDiff::default(), events, init, &out_db).unwrap()
            }
            "mercury_hash" => {
                run(authenticator::MercuryHash::default(), events, init, &out_db).unwrap()
            }
            "mercury_hash_diff" => run(
                authenticator::MercuryHashDiff::default(),
                events,
                init,
                &out_db,
            )
            .unwrap(),
            "merkle" => run(authenticator::Merkle::default(), events, init, &out_db).unwrap(),
            "rsa" => run(
                authenticator::Accumulator::<accumulator::rsa::RsaAccumulator>::default(),
                events,
                init,
                &out_db,
            )
            .unwrap(),
            "rsa_cached" => run(
                authenticator::Accumulator::<
                    accumulator::CachingAccumulator<accumulator::RsaAccumulator>,
                >::default(),
                events,
                init,
                &out_db,
            )
            .unwrap(),
            "vanilla_tuf" => {
                run(authenticator::VanillaTuf::default(), events, init, &out_db).unwrap()
            }
            _ => panic!("not valid"),
        };
        let filename = format!("{}.sqlite", authenticator_config);
        let out_path = Path::new(&output_directory).join(filename);
        out_db
            .backup(DB_NAME, out_path, None)
            .expect("backing up db");
    }

    Ok(())
}

#[test]
fn test_pass() {}
