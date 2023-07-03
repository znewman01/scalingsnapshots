# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 10 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 21 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 46 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 100 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 215 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 464 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 1000 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 2154 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 4641 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 10000 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 21544 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 46415 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 100000 --threads 1 --results long.sqlite3
# cargo run --release -- --authenticators mercury_diff,merkle_bpt,hackage,mercury --packages 215443 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators mercury_diff --packages 464158 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators merkle_bpt --packages 464158 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators hackage --packages 464158 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators mercury --packages 464158 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators mercury_diff --packages 1000000 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators merkle_bpt --packages 1000000 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators hackage --packages 1000000 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators mercury --packages 1000000 --threads 1 --results long.sqlite3
set -e
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 2 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 3 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 4 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 5 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 6 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 7 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 8 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 21 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 46 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 100 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 215 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 464 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 1000 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 2154 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 4641 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 10000 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 21544 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 46415 --threads 1 --results long.sqlite3
cargo run --release -- --authenticators rsa,rsa_pool --packages 100000 --threads 1 --results long.sqlite3
