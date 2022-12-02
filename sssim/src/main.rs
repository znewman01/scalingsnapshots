#![feature(stdin_forwarders)]
#![cfg_attr(feature = "strict", deny(warnings))]
use std::collections::VecDeque;
use std::fmt::Debug;
use std::io;
use std::path::PathBuf;
use time::Duration;

use clap::Parser;
use rusqlite::Connection;
use serde::Serialize;
use uom::si::information::byte;

use sssim::authenticator::{BatchClientSnapshot, ClientSnapshot};
use sssim::log::{Entry, PackageId};
use sssim::simulator::ResourceUsage;
use sssim::util::{DataSized, Information};
use sssim::{authenticator, Authenticator, PoolAuthenticator};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// The number of packages to simulate.
    #[clap(long)]
    packages: usize,
    /// Which authenticators to run (comma-separated)?
    #[clap(long)]
    authenticators: Option<String>,
    /// Path to the database to use for results (sqlite3 format).
    #[clap(long)]
    results: PathBuf,
    /// Name of the dataset
    #[clap(long)]
    dataset: Option<String>,
    /// Number of threads
    #[clap(long, default_value = "1")]
    threads: usize,
}

#[derive(Debug, Serialize)]
struct Event {
    entry: Entry,
    result: ResourceUsage,
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
        "CREATE TABLE IF NOT EXISTS precompute_results (
             id                 INTEGER PRIMARY KEY AUTOINCREMENT,
             technique          TEXT NOT NULL,
             packages           INTEGER,
             server_time_ns     INTEGER,
             server_state_bytes INTEGER,
             cdn_size_bytes     INTEGER,
             cores              INTEGER,
             dataset            TEXT
        )",
        [],
    )?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS update_results (
             id                 INTEGER PRIMARY KEY AUTOINCREMENT,
             technique          TEXT NOT NULL,
             packages           INTEGER,
             server_time_ns     INTEGER,
             server_state_bytes INTEGER,
             cdn_size_bytes     INTEGER,
             batch_size         INTEGER,
             merge_time         INTEGER,
             cores              INTEGER,
             dataset            TEXT
         )",
        [],
    )?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS download_results (
             id              INTEGER PRIMARY KEY AUTOINCREMENT,
             technique       TEXT NOT NULL,
             packages        INTEGER,
             user_time_ns    INTEGER,
             bandwidth_bytes INTEGER,
             dataset         TEXT
         )",
        [],
    )?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS refresh_results (
             id                 INTEGER PRIMARY KEY AUTOINCREMENT,
             technique          TEXT NOT NULL,
             packages           INTEGER,
             elapsed_releases   INTEGER, -- null => initial refresh
             user_time_ns       INTEGER,
             bandwidth_bytes    INTEGER,
             user_state_bytes   INTEGER,
             dataset            TEXT
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
    dataset: &Option<String>,
) -> rusqlite::Result<usize> {
    let time_ns: u64 = time.whole_nanoseconds().try_into().unwrap();
    let server_state_bytes: u64 = server_state.get::<byte>();
    let cdn_size_bytes: u64 = cdn_size.get::<byte>();
    db.execute(
        "
        INSERT INTO precompute_results (
            technique,
            packages,
            server_time_ns,
            server_state_bytes,
            cdn_size_bytes,
            cores,
            dataset
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7) ",
        rusqlite::params![
            technique,
            packages,
            time_ns,
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
    merge_time: u64,
    cores: u16,
    dataset: &Option<String>,
) -> rusqlite::Result<usize> {
    let time_ns: u64 = time.whole_nanoseconds().try_into().unwrap();
    let server_state_bytes = server_state.get::<byte>();
    let cdn_size_bytes = cdn_size.get::<byte>(); //
    db.execute(
        "
        INSERT INTO update_results (
            technique,
            packages,
            server_time_ns,
            server_state_bytes,
            cdn_size_bytes,
            batch_size,
            merge_time,
            cores,
            dataset
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ",
        rusqlite::params![
            technique,
            packages,
            time_ns,
            server_state_bytes,
            cdn_size_bytes,
            batch_size,
            merge_time,
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
    user_state: Information,
    dataset: &Option<String>,
) -> rusqlite::Result<usize> {
    let time_ns: u64 = time.whole_nanoseconds().try_into().unwrap();
    let bandwidth_bytes: u64 = bandwidth.get::<byte>();
    let user_state_bytes: u64 = user_state.get::<byte>();
    db.execute(
        "
        INSERT INTO refresh_results (
            technique,
            packages,
            elapsed_releases,
            user_time_ns,
            bandwidth_bytes,
            user_state_bytes,
            dataset
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7) ",
        rusqlite::params![
            technique,
            packages,
            elapsed_releases,
            time_ns,
            bandwidth_bytes,
            user_state_bytes,
            dataset
        ],
    )
}

fn batch_update_trials<A, S>(
    num_trials: u16,
    auth: &A,
    batch_size: u16,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
) -> rusqlite::Result<()>
where
    S: BatchClientSnapshot + Clone + Default + Debug + DataSized,
    <S as BatchClientSnapshot>::BatchProof: Serialize,
    A: PoolAuthenticator<S> + Clone + Debug + DataSized + Authenticator<S>,
{
    for i in 0..num_trials {
        let cores = 1;
        let mut auth = auth.clone();
        let package_id = PackageId::from("new_package".to_string());
        let (update_time, _) = Duration::time_fn(|| {
            auth.publish(package_id);
        });

        let mut merge_time = Duration::ZERO;
        if i % batch_size == 0 {
            (merge_time, _) = Duration::time_fn(|| {
                auth.batch_process();
            });
        }

        let cdn_size = Information::new::<byte>(0); // TODO: CDN size
                                                    //TODO: write this function that includes merge_time
        insert_update_result(
            db,
            A::name(),
            num_packages,
            update_time,
            auth.size(),
            cdn_size,
            batch_size,
            merge_time.whole_nanoseconds().try_into().unwrap(),
            cores,
            dataset,
        )?;
    }
    Ok(())
}

fn update_trials<A, S>(
    num_trials: u16,
    auth: &A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
) -> rusqlite::Result<()>
where
    S: ClientSnapshot + Clone + Default + Debug + DataSized,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Clone + Debug,
{
    for _ in 0..num_trials {
        // TODO: batches: 0/batch_size
        let batch_size = 1;
        let cores = 1;
        let mut auth = auth.clone();
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
            Duration::ZERO.whole_nanoseconds().try_into().unwrap(),
            cores,
            dataset,
        )?;
    }

    Ok(())
}

fn precompute_trials<A, S>(
    num_trials: u16,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
    packages: &Vec<PackageId>,
) -> rusqlite::Result<A>
where
    S: ClientSnapshot + Clone + Default + Debug + DataSized,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Clone + Debug,
{
    let mut auth = None;
    for _ in 0..num_trials {
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
    // auth: A = auth.clone().take().unwrap();

    Ok(auth.unwrap())
}

fn create_user_state<A, S>(
    num_trials: u16,
    auth: &A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
) -> rusqlite::Result<S>
where
    S: ClientSnapshot + Clone + Default + Debug + DataSized,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Clone + Debug,
{
    let mut user_state_initial: Option<S> = None;
    for _ in 0..num_trials {
        let user_state = auth.get_metadata();
        insert_refresh_result(
            db,
            A::name(),
            num_packages,
            None,
            Duration::ZERO,
            user_state.size(),
            user_state.size(),
            dataset,
        )?;
        user_state_initial.replace(user_state);
    }
    let user_state_initial: S = user_state_initial.take().unwrap();
    Ok(user_state_initial)
}

fn refresh_user_state<A, S>(
    refresh_trials: u16,
    auth_ref: &A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
    user_state_initial: S,
) -> rusqlite::Result<()>
where
    S: ClientSnapshot + Clone + Default + Debug + DataSized,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Clone + Debug,
{
    let mut elapsed_releases = VecDeque::from(vec![0, 1, 10]); // assume sorted
    let max_entry = elapsed_releases[elapsed_releases.len() - 1];
    for idx in 0..=max_entry {
        let mut auth = auth_ref.clone();
        if idx == elapsed_releases[0] {
            for _ in 0..refresh_trials {
                let mut user_state = user_state_initial.clone();
                let maybe_diff = auth.refresh_metadata(user_state.id());
                let (bandwidth, user_time) = match maybe_diff {
                    Some(diff) => {
                        let bandwidth = diff.size();
                        let (user_time, _) = Duration::time_fn(|| {
                            user_state.check_no_rollback(&diff);
                            user_state.update(diff);
                        });
                        (bandwidth, user_time)
                    }
                    None => (Information::new::<byte>(0), Duration::ZERO),
                };
                insert_refresh_result(
                    db,
                    A::name(),
                    num_packages,
                    Some(idx),
                    user_time,
                    bandwidth,
                    user_state.size(),
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
    Ok(())
}

fn download_trials<A, S>(
    download_trials: u16,
    auth: A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
    packages: Vec<PackageId>,
) -> rusqlite::Result<()>
where
    S: ClientSnapshot + Clone + Default + Debug + DataSized,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Clone + Debug,
{
    let mut rng = rand::thread_rng();
    for _ in 1..download_trials {
        let mut auth = auth.clone();
        let user_state = auth.get_metadata();
        let package = rand::seq::SliceRandom::choose(packages.as_slice(), &mut rng).unwrap();

        let (revision, proof) = auth.request_file(user_state.id(), &package);
        let bandwidth = proof.size();

        let (user_time, _) =
            Duration::time_fn(|| user_state.verify_membership(&package, revision, proof));

        insert_download_result(db, A::name(), num_packages, user_time, bandwidth, dataset)?;
    }

    Ok(())
}

fn insert_download_result(
    db: &Connection,
    technique: &str,
    packages: usize,
    time: Duration,
    bandwidth: Information,
    dataset: &Option<String>,
) -> rusqlite::Result<usize> {
    let time_ns: u64 = time.whole_nanoseconds().try_into().unwrap();
    let bandwidth_bytes: u64 = bandwidth.get::<byte>();
    db.execute(
        "
        INSERT INTO download_results (
            technique,
            packages,
            user_time_ns,
            bandwidth_bytes,
            dataset
        ) VALUES ( ?1, ?2, ?3, ?4, ?5) ",
        rusqlite::params![technique, packages, time_ns, bandwidth_bytes, dataset],
    )
}

fn run<S, A>(
    dataset: &Option<String>,
    packages: Vec<PackageId>,
    db: &Connection,
) -> rusqlite::Result<()>
where
    S: ClientSnapshot + Clone + Default + Debug + DataSized,
    <S as ClientSnapshot>::Diff: Serialize,
    A: Authenticator<S> + Clone + Debug,
{
    static PRECOMPUTE_TRIALS: u16 = 3;
    static UPDATE_TRIALS: u16 = 3;
    static REFRESH_TRIALS: u16 = 3;
    static DOWNLOAD_TRIALS: u16 = 3;

    let num_packages = packages.len();

    let auth: A = precompute_trials(PRECOMPUTE_TRIALS, dataset, num_packages, db, &packages)?;

    update_trials(UPDATE_TRIALS, &auth, dataset, num_packages, db)?;

    let user_state_initial = create_user_state(REFRESH_TRIALS, &auth, dataset, num_packages, db)?;

    refresh_user_state(
        REFRESH_TRIALS,
        &auth,
        dataset,
        num_packages,
        db,
        user_state_initial,
    )?;

    download_trials(DOWNLOAD_TRIALS, auth, dataset, num_packages, db, packages)?;

    Ok(())
}

fn run_batch<S, A>(
    dataset: &Option<String>,
    packages: Vec<PackageId>,
    db: &Connection,
) -> rusqlite::Result<()>
where
    S: BatchClientSnapshot + Clone + Default + Debug + DataSized,
    <S as BatchClientSnapshot>::BatchProof: Serialize,
    <S as ClientSnapshot>::Diff: Serialize,
    A: PoolAuthenticator<S> + Clone + Debug,
{
    static PRECOMPUTE_TRIALS: u16 = 3;
    static UPDATE_TRIALS: u16 = 3;
    static REFRESH_TRIALS: u16 = 3;
    static DOWNLOAD_TRIALS: u16 = 3;

    let num_packages = packages.len();
    //TODO: don't hard code
    let batch_size: u16 = 5;

    let auth: A = precompute_trials(PRECOMPUTE_TRIALS, dataset, num_packages, db, &packages)?;

    batch_update_trials(UPDATE_TRIALS, &auth, batch_size, dataset, num_packages, db)?;

    let user_state_initial = create_user_state(REFRESH_TRIALS, &auth, dataset, num_packages, db)?;

    refresh_user_state(
        REFRESH_TRIALS,
        &auth,
        dataset,
        num_packages,
        db,
        user_state_initial,
    )?;

    download_trials(DOWNLOAD_TRIALS, auth, dataset, num_packages, db, packages)?;

    Ok(())
}

fn main() -> io::Result<()> {
    let args: Args = Args::parse();

    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()
        .unwrap();

    let authenticators: Vec<String> = match args.authenticators {
        Some(authenticators) => authenticators.split(",").map(String::from).collect(),
        None => vec![
            "insecure",
            "hackage",
            "mercury_diff",
            "mercury_hash",
            "mercury_hash_diff",
            "merkle",
            "rsa",
            "vanilla_tuf",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    };
    let packages: Vec<_> = (0..args.packages)
        .map(|i| format!("package{i}"))
        .map(PackageId::from)
        .collect();

    let db = Connection::open(&args.results).expect("creating SQLite db");
    create_tables(&db).unwrap();
    for authenticator in authenticators.into_iter() {
        println!("authenticator: {}", authenticator);

        let packages = packages.clone();
        let dataset = &args.dataset;
        match authenticator.as_str() {
            "insecure" => run::<_, authenticator::Insecure>(dataset, packages, &db),
            "hackage" => run::<_, authenticator::Hackage>(dataset, packages, &db),
            "mercury_diff" => run::<_, authenticator::MercuryDiff>(dataset, packages, &db),
            "mercury_hash" => run::<_, authenticator::MercuryHash>(dataset, packages, &db),
            "mercury_hash_diff" => run::<_, authenticator::MercuryHashDiff>(dataset, packages, &db),
            "merkle" => run::<_, authenticator::Merkle>(dataset, packages, &db),
            "rsa" => run::<_, authenticator::Rsa>(dataset, packages, &db),
            "rsa_pool" => run_batch::<_, authenticator::RsaPool>(dataset, packages, &db),
            "vanilla_tuf" => run::<_, authenticator::VanillaTuf>(dataset, packages, &db),
            _ => panic!("not valid"),
        }
        .unwrap();
    }

    Ok(())
}

#[test]
fn test_pass() {}
