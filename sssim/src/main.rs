#![cfg_attr(feature = "strict", deny(warnings))]
use std::collections::VecDeque;
use std::fmt::Debug;
use std::io;
use std::path::PathBuf;
use time::Duration;

use clap::Parser;
use rusqlite::Connection;
use uom::si::information::byte;

use sssim::authenticator::Authenticator;
use sssim::log::PackageId;
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
    /// Number of threads
    #[clap(long, default_value = "1")]
    threads: usize,
}

trait Table {
    fn create(db: &Connection) -> rusqlite::Result<()>;

    fn insert<A: Authenticator>(&self, db: &Connection) -> rusqlite::Result<usize>;
}

fn create_tables(db: &Connection) -> rusqlite::Result<()> {
    OverallTimeResult::create(db)?;
    PrecomputeResult::create(db)?;
    UpdateResult::create(db)?;
    MergeResult::create(db)?;
    RefreshResult::create(db)?;
    DownloadResult::create(db)?;
    Ok(())
}

fn duration_to_ns(duration: Duration) -> u64 {
    duration.whole_nanoseconds().try_into().unwrap()
}

#[derive(Debug)]
struct OverallTimeResult {
    runtime: Duration,
    packages: usize,
    cores: usize,
}

impl Table for OverallTimeResult {
    fn create(db: &Connection) -> rusqlite::Result<()> {
        db.execute(
            "CREATE TABLE IF NOT EXISTS overall_time (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            technique  TEXT,
            runtime_ns INTEGER,
            packages   INTEGER,
            cores      INTEGER
        )",
            [],
        )?;
        Ok(())
    }

    fn insert<A: Authenticator>(&self, db: &Connection) -> rusqlite::Result<usize> {
        let runtime_ns: u64 = duration_to_ns(self.runtime);
        db.execute(
            "
            INSERT INTO overall_time (
                runtime_ns,
                technique,
                packages,
                cores
            ) VALUES ( ?1, ?2, ?3, ?4 )",
            rusqlite::params![runtime_ns, A::name(), self.packages, self.cores],
        )
    }
}

struct PrecomputeResult {
    packages: usize,
    time: Duration,
    server_state: Information,
    cdn_size: Information,
    cores: usize,
}

impl Table for PrecomputeResult {
    fn create(db: &Connection) -> rusqlite::Result<()> {
        db.execute(
            "CREATE TABLE IF NOT EXISTS precompute_results (
             id                 INTEGER PRIMARY KEY AUTOINCREMENT,
             technique          TEXT,
             packages           INTEGER,
             server_time_ns     INTEGER,
             server_state_bytes INTEGER,
             cdn_size_bytes     INTEGER,
             cores              INTEGER
        )",
            [],
        )?;
        Ok(())
    }

    fn insert<A: Authenticator>(&self, db: &Connection) -> rusqlite::Result<usize> {
        db.execute(
            "
        INSERT INTO precompute_results (
            technique,
            packages,
            server_time_ns,
            server_state_bytes,
            cdn_size_bytes,
            cores
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 ) ",
            rusqlite::params![
                A::name(),
                self.packages,
                duration_to_ns(self.time),
                self.server_state.get::<byte>(),
                self.cdn_size.get::<byte>(),
                self.cores,
            ],
        )
    }
}

struct UpdateResult {
    packages: usize,
    time: Duration,
    server_state: Information,
    cdn_size: Information,
    batch_size: u16,
    cores: usize,
}

impl Table for UpdateResult {
    fn create(db: &Connection) -> rusqlite::Result<()> {
        db.execute(
            "CREATE TABLE IF NOT EXISTS update_results (
             id                 INTEGER PRIMARY KEY AUTOINCREMENT,
             technique          TEXT,
             packages           INTEGER,
             server_time_ns     INTEGER,
             server_state_bytes INTEGER,
             cdn_size_bytes     INTEGER,
             batch_size         INTEGER,
             cores              INTEGER
         )",
            [],
        )?;
        Ok(())
    }

    fn insert<A: Authenticator>(&self, db: &Connection) -> rusqlite::Result<usize> {
        db.execute(
            "
        INSERT INTO update_results (
            technique,
            packages,
            server_time_ns,
            server_state_bytes,
            cdn_size_bytes,
            batch_size,
            cores
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7 ) ",
            rusqlite::params![
                A::name(),
                self.packages,
                duration_to_ns(self.time),
                self.server_state.get::<byte>(),
                self.cdn_size.get::<byte>(),
                self.batch_size,
                self.cores,
            ],
        )
    }
}

struct MergeResult {
    packages: usize,
    server_state: Information,
    cdn_size: Information,
    merge_time: Duration,
    batch_size: u16,
    cores: usize,
}

impl Table for MergeResult {
    fn create(db: &Connection) -> rusqlite::Result<()> {
        db.execute(
            "CREATE TABLE IF NOT EXISTS merge_results (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            technique           TEXT,
            packages            INTEGER,
            server_state_bytes  INTEGER,
            cdn_size_bytes      INTEGER,
            merge_time          INTEGER,
            batch_size          INTEGER,
            cores               INTEGER
        )",
            [],
        )?;
        Ok(())
    }

    fn insert<A: Authenticator>(&self, db: &Connection) -> rusqlite::Result<usize> {
        db.execute(
            "
        INSERT INTO merge_results (
            technique,
            packages,
            server_state_bytes,
            merge_time,
            cdn_size_bytes,
            batch_size,
            cores
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7 ) ",
            rusqlite::params![
                A::name(),
                self.packages,
                self.server_state.get::<byte>(),
                duration_to_ns(self.merge_time),
                self.cdn_size.get::<byte>(),
                self.batch_size,
                self.cores,
            ],
        )
    }
}

struct RefreshResult {
    packages: usize,
    elapsed_releases: Option<usize>,
    time: Duration,
    bandwidth: Information,
    user_state: Information,
    cores: usize,
}

impl Table for RefreshResult {
    fn create(db: &Connection) -> rusqlite::Result<()> {
        db.execute(
            "CREATE TABLE IF NOT EXISTS refresh_results (
             id                 INTEGER PRIMARY KEY AUTOINCREMENT,
             technique          TEXT,
             packages           INTEGER,
             elapsed_releases   INTEGER, -- null => initial refresh
             user_time_ns       INTEGER,
             bandwidth_bytes    INTEGER,
             user_state_bytes   INTEGER,
             cores              INTEGER
         )",
            [],
        )?;
        Ok(())
    }

    fn insert<A: Authenticator>(&self, db: &Connection) -> rusqlite::Result<usize> {
        db.execute(
            "
        INSERT INTO refresh_results (
            technique,
            packages,
            elapsed_releases,
            user_time_ns,
            bandwidth_bytes,
            user_state_bytes,
            cores
        ) VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7 ) ",
            rusqlite::params![
                A::name(),
                self.packages,
                self.elapsed_releases,
                duration_to_ns(self.time),
                self.bandwidth.get::<byte>(),
                self.user_state.get::<byte>(),
                self.cores
            ],
        )
    }
}

fn batch_update_trials<A>(
    num_trials: u16,
    auth: &A,
    batch_size: u16,
    num_packages: usize,
    cores: usize,
    db: &Connection,
) -> rusqlite::Result<()>
where
    A: PoolAuthenticator + Clone + Debug + DataSized,
{
    println!("{num_trials} publish trials");
    for i in 0..num_trials {
        println!("trial {i}");
        let mut auth = auth.clone();
        for b in 0..batch_size {
            let package_id = PackageId::from(format!("new_package{b}"));
            let (update_time, _) = Duration::time_fn(|| {
                auth.publish(package_id);
            });
            let cdn_size = auth.cdn_size();
            let result = UpdateResult {
                packages: num_packages,
                time: update_time,
                server_state: auth.size(),
                cdn_size,
                batch_size: b + 1,
                cores,
            };
            result.insert::<A>(db)?;
        }

        let (merge_time, _) = Duration::time_fn(|| {
            auth.batch_process();
        });
        let cdn_size = auth.cdn_size();
        let result = MergeResult {
            packages: num_packages,
            server_state: auth.size(),
            cdn_size,
            merge_time,
            batch_size,
            cores,
        };
        result.insert::<A>(db)?;
    }
    Ok(())
}

fn update_trials<A>(
    num_trials: u16,
    auth: &A,
    num_packages: usize,
    cores: usize,
    db: &Connection,
) -> rusqlite::Result<()>
where
    A: Authenticator + Clone + Debug,
{
    println!("{num_trials} trials");
    for i in 0..num_trials {
        println!("trial {i}");
        let batch_size = 1;
        let mut auth = auth.clone();
        let package_id = PackageId::from("new_package".to_string());
        let (update_time, _) = Duration::time_fn(|| {
            auth.publish(package_id);
        });

        let cdn_size = auth.cdn_size();
        let result = UpdateResult {
            packages: num_packages,
            time: update_time,
            server_state: auth.size(),
            cdn_size,
            batch_size,
            cores,
        };
        result.insert::<A>(db)?;
    }

    Ok(())
}

fn precompute_trials<A>(
    num_trials: u16,
    db: &Connection,
    packages: &[PackageId],
    cores: usize,
) -> rusqlite::Result<A>
where
    A: Authenticator + Debug,
{
    let mut auth = None;
    let num_packages = packages.len();
    println!("{num_trials} trials");
    for i in 0..num_trials {
        println!("trial number: {i}");
        // TODO(maybe): more hooks for progress reporting in batch_import
        let packages = packages.to_owned();
        let (precompute_time, inner_auth) = Duration::time_fn(|| A::batch_import(packages));
        let cdn_size = inner_auth.cdn_size();
        let result = PrecomputeResult {
            packages: num_packages,
            time: precompute_time,
            server_state: inner_auth.size(),
            cdn_size,
            cores,
        };
        result.insert::<A>(db)?;
        auth.replace(inner_auth);
    }

    Ok(auth.unwrap())
}

fn create_user_state<A: Authenticator>(
    num_trials: u16,
    auth: &A,
    num_packages: usize,
    cores: usize,
    db: &Connection,
) -> rusqlite::Result<A::ClientSnapshot> {
    let mut user_state_initial: Option<A::ClientSnapshot> = None;
    println!("{num_trials} trials");
    for i in 0..num_trials {
        println!("trial {i}");
        let user_state = auth.get_metadata();
        let result = RefreshResult {
            packages: num_packages,
            elapsed_releases: None,
            time: Duration::ZERO,
            bandwidth: user_state.size(),
            user_state: user_state.size(),
            cores,
        };
        result.insert::<A>(db)?;
        user_state_initial.replace(user_state);
    }
    let user_state_initial = user_state_initial.take().unwrap();
    Ok(user_state_initial)
}

fn refresh_user_state<A: Authenticator + Clone>(
    refresh_trials: u16,
    auth_ref: &A,
    num_packages: usize,
    db: &Connection,
    user_state_initial: A::ClientSnapshot,
    cores: usize,
) -> rusqlite::Result<()> {
    println!("refresh_user_state");
    let mut elapsed_releases =
        VecDeque::from(vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000]); // assume sorted
    let max_entry: usize =
        std::cmp::min(elapsed_releases[elapsed_releases.len() - 1], num_packages);
    let bar = ProgressBar::new(max_entry.try_into().unwrap());
    let mut auth = auth_ref.clone();
    for idx in 0..=max_entry {
        bar.inc(1);
        if idx == elapsed_releases[0] {
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
                let result = RefreshResult {
                    packages: num_packages,
                    elapsed_releases: Some(idx),
                    time: user_time,
                    bandwidth,
                    user_state: user_state.size(),
                    cores,
                };
                result.insert::<A>(db)?;
            }
            elapsed_releases.pop_front();
            if elapsed_releases.is_empty() {
                break;
            }
        }
        let package = PackageId::from(format!("new_package{idx}"));
        auth.publish(package);
    }
    bar.finish();
    Ok(())
}

fn download_trials<A>(
    download_trials: u16,
    auth: A,
    num_packages: usize,
    db: &Connection,
    packages: Vec<PackageId>,
    cores: usize,
) -> rusqlite::Result<()>
where
    A: Authenticator + Clone + Debug,
{
    let mut rng = rand::thread_rng();
    println!("{download_trials} trials");
    for i in 0..download_trials {
        println!("trial {i}");
        let mut auth = auth.clone();
        let user_state = auth.get_metadata();
        let package = rand::seq::SliceRandom::choose(packages.as_slice(), &mut rng).unwrap();

        let (revision, proof) = auth.request_file(A::id(&user_state), package);
        let bandwidth = proof.size();

        let (user_time, _) =
            Duration::time_fn(|| A::verify_membership(&user_state, package, revision, proof));

        let result = DownloadResult {
            packages: num_packages,
            time: user_time,
            bandwidth,
            cores,
        };
        result.insert::<A>(db)?;
    }

    Ok(())
}

struct DownloadResult {
    packages: usize,
    time: Duration,
    bandwidth: Information,
    cores: usize,
}

impl Table for DownloadResult {
    fn create(db: &Connection) -> rusqlite::Result<()> {
        db.execute(
            "CREATE TABLE IF NOT EXISTS download_results (
             id              INTEGER PRIMARY KEY AUTOINCREMENT,
             technique       TEXT,
             packages        INTEGER,
             user_time_ns    INTEGER,
             bandwidth_bytes INTEGER,
             cores           INTEGER
         )",
            [],
        )?;
        Ok(())
    }

    fn insert<A: Authenticator>(&self, db: &Connection) -> rusqlite::Result<usize> {
        db.execute(
            "
        INSERT INTO download_results (
            technique,
            packages,
            user_time_ns,
            bandwidth_bytes,
            cores
        ) VALUES ( ?1, ?2, ?3, ?4, ?5 ) ",
            rusqlite::params![
                A::name(),
                self.packages,
                duration_to_ns(self.time),
                self.bandwidth.get::<byte>(),
                self.cores
            ],
        )
    }
}
fn run<A>(
    packages: Vec<PackageId>,
    db: &Connection,
    cores: usize,
) -> rusqlite::Result<OverallTimeResult>
where
    A: Authenticator + Clone + Debug,
{
    let num_packages = packages.len();
    let (runtime, err) = Duration::time_fn(|| {
        static PRECOMPUTE_TRIALS: u16 = 1;
        static UPDATE_TRIALS: u16 = 1;
        static REFRESH_TRIALS: u16 = 1;
        static DOWNLOAD_TRIALS: u16 = 1;

        println!("precompute");
        let auth: A = precompute_trials(PRECOMPUTE_TRIALS, db, &packages, cores)?;

        println!("update");
        update_trials(UPDATE_TRIALS, &auth, num_packages, cores, db)?;

        println!("refresh");
        let user_state_initial = create_user_state(REFRESH_TRIALS, &auth, num_packages, cores, db)?;

        refresh_user_state(
            REFRESH_TRIALS,
            &auth,
            num_packages,
            db,
            user_state_initial,
            cores,
        )?;

        println!("download");
        download_trials(DOWNLOAD_TRIALS, auth, num_packages, db, packages, cores)?;
        Ok(())
    });
    err.map(|_| OverallTimeResult {
        runtime,
        packages: num_packages,
        cores,
    })
}

fn run_batch<A>(
    packages: Vec<PackageId>,
    db: &Connection,
    batch_sizes: Vec<u16>,
    cores: usize,
) -> rusqlite::Result<OverallTimeResult>
where
    A: PoolAuthenticator + Clone + Debug,
{
    let num_packages = packages.len();
    let (runtime, err) = Duration::time_fn(|| {
        static PRECOMPUTE_TRIALS: u16 = 1;
        static UPDATE_TRIALS: u16 = 1;
        static REFRESH_TRIALS: u16 = 1;
        static DOWNLOAD_TRIALS: u16 = 1;

        println!("precompute");
        let auth: A = precompute_trials(PRECOMPUTE_TRIALS, db, &packages, cores)?;

        for batch_size in batch_sizes {
            println!("batch_size: {batch_size}");
            batch_update_trials(UPDATE_TRIALS, &auth, batch_size, num_packages, cores, db)?;
        }

        println!("refresh");
        let user_state_initial = create_user_state(REFRESH_TRIALS, &auth, num_packages, cores, db)?;

        refresh_user_state(
            REFRESH_TRIALS,
            &auth,
            num_packages,
            db,
            user_state_initial,
            cores,
        )?;

        println!("download");
        download_trials(DOWNLOAD_TRIALS, auth, num_packages, db, packages, cores)?;

        Ok(())
    });
    err.map(|_| OverallTimeResult {
        runtime,
        packages: num_packages,
        cores,
    })
}

fn main() -> io::Result<()> {
    let args: Args = Args::parse();

    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()
        .unwrap();

    let authenticators: Vec<String> = match args.authenticators {
        Some(authenticators) => authenticators.split(',').map(String::from).collect(),
        None => vec![
            "insecure",
            "hackage",
            "mercury_diff",
            "sparse_merkle",
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
        println!("\nauthenticator: {authenticator}");

        let packages = packages.clone();
        let batch_sizes = if args.threads == 1 {
            vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000]
        } else {
            vec![100]
        };
        let result = match authenticator.as_str() {
            "insecure" => run::<authenticator::Insecure>(packages, &db, args.threads),
            "hackage" => run::<authenticator::Hackage>(packages, &db, args.threads),
            "mercury_diff" => run::<authenticator::MercuryDiff>(packages, &db, args.threads),
            "sparse_merkle" => run::<authenticator::SparseMerkle>(packages, &db, args.threads),
            "rsa" => run::<authenticator::Rsa>(packages, &db, args.threads),
            // TODO(must): try with different batch sizes
            "rsa_pool" => {
                run_batch::<authenticator::RsaPool>(packages, &db, batch_sizes, args.threads)
            }
            "mercury" => run::<authenticator::VanillaTuf>(packages, &db, args.threads),
            _ => panic!("not valid"),
        }
        .unwrap();
        dbg!(&result);
        match authenticator.as_str() {
            "insecure" => result.insert::<authenticator::Insecure>(&db),
            "hackage" => result.insert::<authenticator::Hackage>(&db),
            "mercury_diff" => result.insert::<authenticator::MercuryDiff>(&db),
            "sparse_merkle" => result.insert::<authenticator::SparseMerkle>(&db),
            "rsa" => result.insert::<authenticator::Rsa>(&db),
            "rsa_pool" => result.insert::<authenticator::RsaPool>(&db),
            "mercury" => result.insert::<authenticator::VanillaTuf>(&db),
            _ => panic!("not valid"),
        }
        .unwrap();
    }

    Ok(())
}

#[test]
fn test_pass() {}
