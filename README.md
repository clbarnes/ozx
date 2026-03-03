# ozx

A toy CLI for archving an [OME-Zarr](https://ngff.openmicroscopy.org) hierarchy into a .ozx file [per RFC-9](https://ngff.openmicroscopy.org/rfc/9/index.html).

## Usage

Requires a recent [rust toolchain](https://rustup.rs/).

```sh
# Clone and enter this repository
git clone https://github.com/clbarnes/ozx.git
cd ozx

# Optionally, install the `ozx` tool
cargo install --path .
# Alternatively, replace all `ozx` invocations below with `cargo run --`
# or `cargo run --release --` to do it faster

# If you don't have any OME-Zarr data available,
# you can fetch some with
./fetch_kingsnake.sh
# which populates `./fixtures/kingsnake.ome.zarr/`

# See the tool's help text
ozx --help
ozx create --help

# Create an OZX archive, with your data or kingsnake
ozx create --force ./fixtures/kingsnake.ozx ./fixtures/kingsnake.ome.zarr
```

You can also directly install the tool with `cargo install ozx`.

## Limitations

This was originally intended as an exploration of the format rather than a production tool,
and is currently limited to local OME-Zarr stores and runs synchronously on a single thread.

## Attribution

The test data retrieved by `fetch_kingsnake.sh` is as described by [InsightSoftwareConsortium/OMEZarrOpenSciVisDatasets](https://github.com/InsightSoftwareConsortium/OMEZarrOpenSciVisDatasets/tree/main?tab=readme-ov-file#kingsnake).
