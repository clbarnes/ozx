kingsnake:
    /bin/sh ./fetch_kingsnake.sh

prebuild:
    cargo build --release

bench: kingsnake prebuild
    mkdir -p tmp
    rm -f tmp/kingsnake.ozx
    hyperfine --warmup 10 --runs 10 'target/release/ozx create tmp/kingsnake.ozx fixtures/kingsnake' --conclude 'rm tmp/kingsnake.ozx'
