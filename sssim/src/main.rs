#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::prelude::*;
use clap::Parser;
use rusqlite::{backup, Connection, DatabaseName};
use serde::Serialize;
use uom::si::information::byte;

use sssim::authenticator::ClientSnapshot;
use sssim::log::{Action, Entry, PackageId};
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

const DB_NAME: DatabaseName = DatabaseName::Main;

fn write_sqlite(event: Event, conn: &Connection) -> Option<rusqlite::Result<usize>> {
    let (action, user) = match event.entry.action {
        Action::RefreshMetadata { user } => ("refresh", Some(user.0)),
        Action::Download { user, .. } => ("download", Some(user.0)),
        Action::Publish { .. } => ("publish", None),
        Action::Goodbye { .. } => {
            return None;
        }
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
    let statement = conn.prepare_cached(
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
    );
    if let Err(e) = statement {
        return Some(Err(e));
    }
    let mut statement = statement.unwrap();
    Some(statement.execute(rusqlite::params![
        event.entry.timestamp.unix_timestamp(),
        action,
        user,
        server_compute_ns,
        user_compute_ns,
        event.result.bandwidth.get::<byte>(),
        event.result.storage.get::<byte>(),
    ]))
}

fn run<S, A, X, Y>(
    events: X,
    init: Y,
    out: &Connection,
    timing_file: &mut File,
) -> rusqlite::Result<()>
where
    S: ClientSnapshot + Default + Debug,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Debug,
    X: BufRead,
    Y: BufRead,
{
    writeln!(timing_file, "{} start", Utc::now().to_rfc3339()).expect("can't write?");
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

    let bar = if console::Term::stderr().is_term() {
        let bar = indicatif::ProgressBar::new(362350);
        bar.set_message("Initializing");
        Some(bar)
    } else {
        eprintln!("Initializing");
        None
    };
    let mut count = 0;
    let mut last_update = Instant::now();
    let mut to_import = Vec::<PackageId>::new();
    for line in init.lines() {
        count += 1;
        if last_update.elapsed() > Duration::from_secs(1) {
            last_update = Instant::now();
            match &bar {
                Some(bar) => {
                    bar.set_position(count);
                }
                None => {
                    eprintln!("Packages: {}", count);
                }
            };
        }
        let result = serde_json::from_str(&line.expect("reading from file failed"));
        let entry: Entry = result.expect("bad log entry");
        match entry.action {
            Action::Publish { package } => {
                to_import.push(package.id);
            }
            _ => panic!("Initialization should only include publish"),
        }
    }
    let auth = A::batch_import(to_import);
    match bar {
        Some(bar) => {
            bar.finish();
        }
        None => {
            eprintln!("Done initializing!");
        }
    };
    writeln!(timing_file, "{} init-done", Utc::now().to_rfc3339()).expect("can't write?");

    let mut simulator = Simulator::new(auth);
    let bar = if console::Term::stderr().is_term() {
        let bar = indicatif::ProgressBar::new(2269238);
        bar.set_message("Simulating");
        Some(bar)
    } else {
        None
    };
    let mut count = 0;
    let mut last_update = Instant::now();
    for line in events.lines() {
        count += 1;
        if last_update.elapsed() > Duration::from_secs(1) {
            last_update = Instant::now();
            match &bar {
                Some(bar) => {
                    bar.set_position(count);
                }
                None => {
                    eprintln!("Events: {}", count);
                }
            };
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
            if let Some(result) = write_sqlite(event, out) {
                result?;
            }
        }
        let usage = simulator.process(&mut entry.action);
        let event = Event {
            entry,
            result: usage,
        };
        if let Some(result) = write_sqlite(event, out) {
            result?;
        }
    }
    match bar {
        Some(bar) => {
            bar.finish();
        }
        None => {
            eprintln!("Done simulating!");
        }
    };
    writeln!(timing_file, "{} done", Utc::now().to_rfc3339()).expect("can't write?");
    Ok(())
}

fn pg(p: backup::Progress) {
    println!("{}", p.pagecount);
    println!("{}", p.remaining);
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
            "vanilla_tuf".to_string(),
        ],
    };
    let output_directory = args.output_directory.unwrap_or_else(|| ".".to_string());

    for authenticator_config in authenticator_configs.iter() {
        let events = BufReader::new(File::open(args.events_path.clone())?);
        let init = BufReader::new(File::open(args.init_path.clone())?);
        let out_path = {
            let filename = format!("{}.sqlite", authenticator_config);
            Path::new(&output_directory).join(filename)
        };
        let out_db = Connection::open(&out_path).expect("creating SQLite db");
        let mut timing_file = {
            let name = format!("timings-{}", authenticator_config.as_str());
            let path = Path::new(&output_directory).join(name);
            File::create(path)?
        };

        println!("authenticator: {}", authenticator_config);
        match authenticator_config.as_str() {
            "insecure" => {
                run::<_, authenticator::Insecure, _, _>(events, init, &out_db, &mut timing_file)
                    .unwrap()
            }
            "hackage" => {
                run::<_, authenticator::Hackage, _, _>(events, init, &out_db, &mut timing_file)
                    .unwrap()
            }
            "mercury_diff" => {
                run::<_, authenticator::MercuryDiff, _, _>(events, init, &out_db, &mut timing_file)
                    .unwrap()
            }
            "mercury_hash" => {
                run::<_, authenticator::MercuryHash, _, _>(events, init, &out_db, &mut timing_file)
                    .unwrap()
            }
            "mercury_hash_diff" => run::<_, authenticator::MercuryHashDiff, _, _>(
                events,
                init,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "merkle" => {
                run::<_, authenticator::Merkle, _, _>(events, init, &out_db, &mut timing_file)
                    .unwrap()
            }
            "rsa" => run::<_, authenticator::Accumulator<accumulator::rsa::RsaAccumulator>, _, _>(
                events,
                init,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "vanilla_tuf" => {
                run::<_, authenticator::VanillaTuf, _, _>(events, init, &out_db, &mut timing_file)
                    .unwrap()
            }
            _ => panic!("not valid"),
        };
    }

    Ok(())
}

#[test]
fn test_pass() {}
