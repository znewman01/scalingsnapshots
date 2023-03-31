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

use sssim::authenticator::Authenticator;
use sssim::log::{Entry, PackageId};
use sssim::simulator::ResourceUsage;
use sssim::util::{DataSized, Information};
use sssim::{authenticator, PoolAuthenticator};

use indicatif::ProgressBar;

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

fn create_tables(db: &Connection) -> rusqlite::Result<()> {
    db.execute(
        "CREATE TABLE IF NOT EXISTS overall_time (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            runtime_ns INTEGER,
            technique  TEXT NOT NULL,
            packages  INTEGER
        )",
        [],
    )?;
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
             cores              INTEGER,
             dataset            TEXT
         )",
        [],
    )?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS merge_results (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            technique           TEXT NOT NULL,
            packages            INTEGER,
            server_state_bytes  INTEGER,
            cdn_size_bytes      INTEGER,
            merge_time          INTEGER,
            batch_size          INTEGER,
            cores               INTEGER,
            dataset             TEXT
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

fn insert_overall_time(
    db: &Connection,
    runtime: Duration,
    technique: &str,
    packages: usize,
) -> rusqlite::Result<usize> {
    let runtime_ns: u64 = runtime.whole_nanoseconds().try_into().unwrap();
    db.execute(
        "
        INSERT INTO overall_time (
            runtime_ns,
            technique,
            packages
        ) VALUES (?1, ?2, ?3) ",
        rusqlite::params![runtime_ns, technique, packages],
    )
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
            cores,
            dataset
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8 ) ",
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

fn insert_merge_result(
    db: &Connection,
    technique: &str,
    packages: usize,
    server_state: Information,
    cdn_size: Information,
    merge_time: Duration,
    batch_size: u16,
    cores: u16,
    dataset: &Option<String>,
) -> rusqlite::Result<usize> {
    let merge_time_ns: u64 = merge_time.whole_nanoseconds().try_into().unwrap();
    let server_state_bytes = server_state.get::<byte>();
    let cdn_size_bytes = cdn_size.get::<byte>(); //
    db.execute(
        "
        INSERT INTO merge_results (
            technique,
            packages,
            server_state_bytes,
            merge_time,
            cdn_size_bytes,
            batch_size,
            cores,
            dataset
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ",
        rusqlite::params![
            technique,
            packages,
            server_state_bytes,
            merge_time_ns,
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

fn batch_update_trials<A>(
    num_trials: u16,
    auth: &A,
    batch_size: u16,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
) -> rusqlite::Result<()>
where
    A: PoolAuthenticator + Clone + Debug + DataSized,
{
    println!("{num_trials} publish trials");
    for i in 0..num_trials {
        println!("trial {i}");
        // TODO(must): don't hard code cores
        let cores = 1;
        let mut auth = auth.clone();
        for b in 0..batch_size {
            let package_id = PackageId::from(format!("new_package{}", b));
            let (update_time, _) = Duration::time_fn(|| {
                auth.publish(package_id);
            });
            let cdn_size = auth.cdn_size();
            insert_update_result(
                db,
                A::name(),
                num_packages,
                update_time,
                auth.size(),
                cdn_size,
                b + 1,
                cores,
                dataset,
            )?;
        }

        let (merge_time, _) = Duration::time_fn(|| {
            auth.batch_process();
        });
        let cdn_size = auth.cdn_size();
        insert_merge_result(
            db,
            A::name(),
            num_packages,
            auth.size(),
            cdn_size,
            merge_time,
            batch_size,
            cores,
            dataset,
        )?;
    }
    Ok(())
}

fn update_trials<A>(
    num_trials: u16,
    auth: &A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
) -> rusqlite::Result<()>
where
    A: Authenticator + Clone + Debug,
{
    println!("{num_trials} trials");
    for i in 0..num_trials {
        println!("trial {i}");
        let batch_size = 1;
        // TODO(must): don't hard code cores
        let cores = 1;
        let mut auth = auth.clone();
        let package_id = PackageId::from("new_package".to_string());
        let (update_time, _) = Duration::time_fn(|| {
            auth.publish(package_id);
        });

        let cdn_size = auth.cdn_size();
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

    Ok(())
}

fn precompute_trials<A>(
    num_trials: u16,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
    packages: &Vec<PackageId>,
) -> rusqlite::Result<A>
where
    A: Authenticator + Debug,
{
    let mut auth = None;
    println!("{num_trials} trials");
    for i in 0..num_trials {
        println!("trial number: {i}");
        // TODO(maybe): more hooks for progress reporting in batch_import
        let packages = packages.clone();
        let (precompute_time, inner_auth) = Duration::time_fn(|| A::batch_import(packages));
        let cdn_size = inner_auth.cdn_size();
        // TODO(must): don't hard code cores
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

fn create_user_state<A: Authenticator>(
    num_trials: u16,
    auth: &A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
) -> rusqlite::Result<A::ClientSnapshot> {
    let mut user_state_initial: Option<A::ClientSnapshot> = None;
    println!("{num_trials} trials");
    for i in 0..num_trials {
        println!("trial {i}");
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
    let user_state_initial = user_state_initial.take().unwrap();
    Ok(user_state_initial)
}

fn refresh_user_state<A: Authenticator + Clone>(
    refresh_trials: u16,
    auth_ref: &A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
    user_state_initial: A::ClientSnapshot,
) -> rusqlite::Result<()> {
    println!("refresh_user_state");
    let mut elapsed_releases = VecDeque::from(vec![1, 10, 100, 1000]); // assume sorted
    let max_entry: usize =
        std::cmp::min(elapsed_releases[elapsed_releases.len() - 1], num_packages);
    let bar = ProgressBar::new(max_entry.try_into().unwrap());
    let mut auth = auth_ref.clone();
    for idx in 0..=max_entry {
        bar.inc(1);
        if idx == elapsed_releases[0] {
            println!("On {idx} releases");
            for _ in 0..refresh_trials {
                let mut user_state = user_state_initial.clone();
                let maybe_diff = auth.refresh_metadata(A::id(&user_state));
                let (bandwidth, user_time) = match maybe_diff {
                    Some(diff) => {
                        let bandwidth = diff.size();
                        let (user_time, _) = Duration::time_fn(|| {
                            assert!(A::check_no_rollback(&user_state, &diff));
                            A::update(&mut user_state, diff);
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
        let package = PackageId::from(format!("new_package{}", idx));
        auth.publish(package);
    }
    bar.finish();
    Ok(())
}

fn download_trials<A>(
    download_trials: u16,
    auth: A,
    dataset: &Option<String>,
    num_packages: usize,
    db: &Connection,
    packages: Vec<PackageId>,
) -> rusqlite::Result<()>
where
    A: Authenticator + Clone + Debug,
{
    let mut rng = rand::thread_rng();
    println!("{download_trials} trials");
    for i in 1..download_trials {
        println!("trial {i}");
        let mut auth = auth.clone();
        let user_state = auth.get_metadata();
        let package = rand::seq::SliceRandom::choose(packages.as_slice(), &mut rng).unwrap();

        let (revision, proof) = auth.request_file(A::id(&user_state), &package);
        let bandwidth = proof.size();

        let (user_time, _) =
            Duration::time_fn(|| A::verify_membership(&user_state, &package, revision, proof));

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

fn run<A>(
    dataset: &Option<String>,
    packages: Vec<PackageId>,
    db: &Connection,
) -> rusqlite::Result<()>
where
    A: Authenticator + Clone + Debug,
{
    static PRECOMPUTE_TRIALS: u16 = 3;
    static UPDATE_TRIALS: u16 = 3;
    static REFRESH_TRIALS: u16 = 3;
    static DOWNLOAD_TRIALS: u16 = 3;

    let num_packages = packages.len();

    println!("precompute");
    let auth: A = precompute_trials(PRECOMPUTE_TRIALS, dataset, num_packages, db, &packages)?;

    println!("update");
    update_trials(UPDATE_TRIALS, &auth, dataset, num_packages, db)?;

    println!("refresh");
    let user_state_initial = create_user_state(REFRESH_TRIALS, &auth, dataset, num_packages, db)?;

    refresh_user_state(
        REFRESH_TRIALS,
        &auth,
        dataset,
        num_packages,
        db,
        user_state_initial,
    )?;

    println!("download");
    download_trials(DOWNLOAD_TRIALS, auth, dataset, num_packages, db, packages)?;

    Ok(())
}

fn run_batch<A>(
    dataset: &Option<String>,
    packages: Vec<PackageId>,
    db: &Connection,
    batch_sizes: Vec<u16>,
) -> rusqlite::Result<()>
where
    A: PoolAuthenticator + Clone + Debug,
{
    static PRECOMPUTE_TRIALS: u16 = 3;
    static UPDATE_TRIALS: u16 = 3;
    static REFRESH_TRIALS: u16 = 3;
    static DOWNLOAD_TRIALS: u16 = 3;

    let num_packages = packages.len();

    println!("precompute");
    let auth: A = precompute_trials(PRECOMPUTE_TRIALS, dataset, num_packages, db, &packages)?;

    for batch_size in batch_sizes {
        print!("batch_size: {batch_size}\n");
        batch_update_trials(UPDATE_TRIALS, &auth, batch_size, dataset, num_packages, db)?;
    }

    println!("refresh");
    let user_state_initial = create_user_state(REFRESH_TRIALS, &auth, dataset, num_packages, db)?;

    refresh_user_state(
        REFRESH_TRIALS,
        &auth,
        dataset,
        num_packages,
        db,
        user_state_initial,
    )?;

    println!("download");
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
            "rsa_pool",
            "mercury",
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
        println!("\nauthenticator: {}", authenticator);

        let packages = packages.clone();
        let package_len = packages.len();
        let dataset = &args.dataset;
        let (runtime, _) = Duration::time_fn(|| {
            match authenticator.as_str() {
                "insecure" => run::<authenticator::Insecure>(dataset, packages, &db),
                "hackage" => run::<authenticator::Hackage>(dataset, packages, &db),
                "mercury_diff" => run::<authenticator::MercuryDiff>(dataset, packages, &db),
                "mercury_hash" => run::<authenticator::MercuryHash>(dataset, packages, &db),
                "mercury_hash_diff" => {
                    run::<authenticator::MercuryHashDiff>(dataset, packages, &db)
                }
                "merkle" => run::<authenticator::Merkle>(dataset, packages, &db),
                "rsa" => run::<authenticator::Rsa>(dataset, packages, &db),
                // TODO(must): try with different batch sizes
                "rsa_pool" => run_batch::<authenticator::RsaPool>(
                    dataset,
                    packages,
                    &db,
                    vec![1, 10, 50, 100, 500],
                ),
                "mercury" => run::<authenticator::VanillaTuf>(dataset, packages, &db),
                _ => panic!("not valid"),
            }
            .unwrap();
        });
        insert_overall_time(&db, runtime, authenticator.as_str(), package_len);
    }

    Ok(())
}

#[test]
fn test_pass() {}
