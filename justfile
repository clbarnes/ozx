kingsnake:
    /bin/sh ./fetch_kingsnake.sh

prebuild:
    cargo build --release

bench: kingsnake prebuild
    mkdir -p tmp
    hyperfine --warmup 10 --prepare 'rm -f tmp/kingsnake.ozx' --runs 10 'target/release/ozx create tmp/kingsnake.ozx fixtures/kingsnake.ome.zarr'
