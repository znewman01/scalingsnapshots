#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::collections::VecDeque;
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use time::Duration;

use clap::Parser;
use rusqlite::{Connection, DatabaseName};
use serde::Serialize;
use uom::si::information::byte;

use sssim::authenticator::ClientSnapshot;
use sssim::log::{Action, Entry, PackageId};
use sssim::simulator::ResourceUsage;
use sssim::util::{DataSized, Information};
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
    let statement = conn.prepare(
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

// Read packages from file
/*
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
*/

fn create_tables(db: &Connection) -> rusqlite::Result<()> {
    db.execute(
        "CREATE TABLE precompute_results (
                 id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                 technique            TEXT NOT NULL,
                 packages             INTEGER,
                 time_nanos           INTEGER,
                 server_state_bytes   INTEGER,
                 cdn_size_bytes       INTEGER,
                 cores                INTEGER,
                 dataset              TEXT,
                 )",
        [],
    )?;
    db.execute(
        "CREATE TABLE update_results (
                 id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                 technique            TEXT NOT NULL,
                 packages             INTEGER,
                 server_time_nanos    INTEGER,
                 server_state_bytes   INTEGER,
                 cdn_size_bytes       INTEGER,
                 batch_size           INTEGER,
                 cores                INTEGER,
                 dataset              TEXT,
                 )",
        [],
    )?;
    db.execute(
        "CREATE TABLE download_results (
                 id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                 technique            TEXT NOT NULL,
                 packages             INTEGER,
                 user_time_nanos      INTEGER,
                 bandwidth_bytes      INTEGER,
                 dataset              TEXT,
                 )",
        [],
    )?;
    db.execute(
        "CREATE TABLE refresh_results (
                 id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                 technique            TEXT NOT NULL,
                 packages             INTEGER,
                 elapsed_releases     INTEGER, -- null => initial refresh
                 user_time_nanos      INTEGER,
                 bandwidth_bytes      INTEGER,
                 user_storage_bytes   INTEGER,
                 dataset              TEXT,
                 )",
        [],
    )?;
    Ok(())
}

fn insert_precompute_result(
    db: &Connection,
    technique: &str,
    packages: usize,
    time: Duration,
    server_state: Information,
    cdn_size: Information,
    cores: u16,
    dataset: &str,
) -> rusqlite::Result<usize> {
    let time_nanos: u64 = time.whole_nanoseconds().try_into().unwrap();
    let server_state_bytes: u64 = server_state.get::<byte>();
    let cdn_size_bytes: u64 = cdn_size_state.get::<byte>();
    db.execute(
        "
        INSERT INTO precompute_results (
            technique,
            packages,
            server_time_nanos,
            server_state_bytes,
            cdn_size_bytes,
            cores,
            dataset,
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7) ",
        rusqlite::params![
            technique,
            packages,
            time_nanos,
            server_state_bytes,
            cdn_size_bytes,
            cores,
            dataset
        ],
    )
}

fn insert_update_result(
    db: &Connection,
    technique: &str,
    packages: usize,
    time: Duration,
    server_state: Information,
    cdn_size: Information,
    batch_size: u16,
    cores: u16,
    dataset: &str,
) -> rusqlite::Result<usize> {
    let time_ns: u64 = time.whole_nanoseconds().try_into().unwrap();
    let server_state_bytes = server_state.get::<byte>();
    let cdn_size_bytes = cdn_size.get::<byte>(); //
    db.execute(
        "
        INSERT INTO update_results (
            technique,
            packages,
            server_time_nanos,
            server_state_bytes,
            cdn_size_bytes,
            batch_size,
            cores,
            dataset,
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ",
        rusqlite::params![
            technique,
            packages,
            time_ns,
            server_state_bytes,
            cdn_size_bytes,
            batch_size,
            cores,
            dataset
        ],
    )
}

fn insert_refresh_result(
    db: &Connection,
    technique: &str,
    packages: usize,
    elapsed_releases: Option<usize>,
    time: Duration,
    bandwidth: Information,
    user_storage: Information,
    dataset: &str,
) -> rusqlite::Result<usize> {
    let time_ns: u64 = time.whole_nanoseconds().try_into().unwrap();
    let bandwidth_bytes: u64 = bandwidth.get::<byte>();
    let user_storage_bytes: u64 = bandwidth.get::<byte>();
    db.execute(
        "
        INSERT INTO refresh_results (
            technique,
            packages,
            elapsed_releases
            user_time_nanos,
            bandwidth_bytes,
            user_storage_bytes,
            dataset,
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7) ",
        rusqlite::params![
            technique,
            packages,
            elapsed_releases,
            time_ns,
            bandwidth_bytes,
            user_storage_bytes,
            dataset
        ],
    )
}

fn insert_download_result(
    db: &Connection,
    technique: &str,
    packages: usize,
    time: Duration,
    bandwidth: Information,
    dataset: &str,
) -> rusqlite::Result<usize> {
    let time_ns: u64 = time.whole_nanoseconds().try_into().unwrap();
    let bandwidth_bytes: u64 = bandwidth.get::<byte>();
    db.execute(
        "
        INSERT INTO download_results (
            technique,
            packages,
            user_time_nanos,
            bandwidth_bytes,
            dataset,
        ) VALUES ( ?1, ?2, ?3, ?4, ?5) ",
        rusqlite::params![technique, packages, time_ns, bandwidth_bytes, dataset],
    )
}

fn run<S, A, X>(
    dataset: &str,
    events: X,
    packages: Vec<PackageId>,
    db: &Connection,
    timing_file: &mut File,
) -> rusqlite::Result<()>
where
    S: ClientSnapshot + Clone + Default + Debug + DataSized,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Clone + Debug,
    X: BufRead,
{
    static PRECOMPUTE_TRIALS: u16 = 3;
    static UPDATE_TRIALS: u16 = 3;
    static REFRESH_TRIALS: u16 = 3;
    static DOWNLOAD_TRIALS: u16 = 3;

    let num_packages = packages.len();
    let mut auth: Option<A> = None;
    for _ in 0..PRECOMPUTE_TRIALS {
        // TODO: hook for progress reporting in batch_import?
        let packages = packages.clone();
        let (precompute_time, inner_auth) = Duration::time_fn(|| A::batch_import(packages));
        let cdn_size = Information::new::<byte>(0); // TODO: CDN size
        let cores = 1;
        insert_precompute_result(
            db,
            A::name(),
            num_packages,
            precompute_time,
            inner_auth.size(),
            cdn_size,
            cores,
            dataset,
        )?;
        auth.replace(inner_auth);
    }
    let auth: A = auth.clone().take().unwrap();

    for _ in 0..UPDATE_TRIALS {
        // TODO: batches: 0/batch_size
        let batch_size = 1;
        let cores = 1;
        let auth = auth.clone();
        let package_id = PackageId::from("new_package".to_string());
        let (update_time, _) = Duration::time_fn(|| {
            auth.publish(package_id);
        });
        let cdn_size = Information::new::<byte>(0); // TODO: CDN size
        insert_update_result(
            db,
            A::name(),
            num_packages,
            update_time,
            auth.size(),
            cdn_size,
            batch_size,
            cores,
            dataset,
        )?;
    }

    let user_state_initial: Option<S> = None;
    for _ in 0..REFRESH_TRIALS {
        let mut user_state = S::default();
        let diff = auth.refresh_metadata(user_state.id()).unwrap();
        let bandwidth_bytes = diff.size();
        let (user_time, _) = Duration::time_fn(|| {
            user_state.update(diff);
        });
        let user_storage_bytes = user_state.size();
        insert_refresh_result(
            db,
            A::name(),
            num_packages,
            None,
            user_time,
            bandwidth_bytes,
            user_storage_bytes,
            dataset,
        )?;
        user_state_initial.replace(user_state);
    }
    let user_state_initial: S = user_state_initial.take().unwrap();

    let mut elapsed_releases = VecDeque::from(vec![0, 1, 10]); // assume sorted
    {
        let auth = auth.clone();
        let max_entry = elapsed_releases[elapsed_releases.len() - 1];
        for idx in 0..=max_entry {
            if idx == elapsed_releases[0] {
                for _ in 0..REFRESH_TRIALS {
                    let mut user_state = user_state_initial.clone();
                    let diff = auth.refresh_metadata(user_state.id()).unwrap();
                    let bandwidth = diff.size();
                    let (user_time, _) = Duration::time_fn(|| {
                        user_state.check_no_rollback(&diff);
                        user_state.update(diff);
                    });
                    let user_storage = user_state.size();

                    insert_refresh_result(
                        db,
                        A::name(),
                        num_packages,
                        Some(idx),
                        user_time,
                        bandwidth,
                        user_storage,
                        dataset,
                    )?;
                }
                elapsed_releases.pop_front();
                if elapsed_releases.is_empty() {
                    break;
                }
            }
            // TODO: for RSA
            // - how fast in the same day? batch_size=1million
            // - how about N days? batch_size=1
            // - can use many cores
            let package = PackageId::from(format!("new_package{}", idx));
            auth.publish(package);
        }
    }

    let mut rng = rand::thread_rng();
    for _ in 1..DOWNLOAD_TRIALS {
        let mut user_state = S::default();
        let diff = auth.refresh_metadata(user_state.id()).unwrap();
        user_state.update(diff);
        let package = rand::seq::SliceRandom::choose(packages.as_slice(), &mut rng).unwrap();

        let (revision, proof) = auth.request_file(user_state.id(), &package);

        let (user_time, _) =
            Duration::time_fn(|| user_state.verify_membership(&package, revision, proof));
        let bandwidth = proof.size();

        insert_download_result(db, A::name(), num_packages, user_time, bandwidth, dataset)?;
    }

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
        out_db.execute("PRAGMA synchronous=OFF", []).unwrap();
        create_tables(&out_db).unwrap();
        let mut timing_file = {
            let name = format!("timings-{}", authenticator_config.as_str());
            let path = Path::new(&output_directory).join(name);
            File::create(path)?
        };

        let dataset = ""; // TODO
        let packages = vec![]; // TODO
        println!("authenticator: {}", authenticator_config);
        match authenticator_config.as_str() {
            "insecure" => run::<_, authenticator::Insecure, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "hackage" => run::<_, authenticator::Hackage, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "mercury_diff" => run::<_, authenticator::MercuryDiff, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "mercury_hash" => run::<_, authenticator::MercuryHash, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "mercury_hash_diff" => run::<_, authenticator::MercuryHashDiff, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "merkle" => run::<_, authenticator::Merkle, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "rsa" => run::<_, authenticator::Accumulator<accumulator::rsa::RsaAccumulator>, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            "vanilla_tuf" => run::<_, authenticator::VanillaTuf, _>(
                dataset,
                events,
                packages,
                &out_db,
                &mut timing_file,
            )
            .unwrap(),
            _ => panic!("not valid"),
        };
    }

    Ok(())
}

#[test]
fn test_pass() {}
