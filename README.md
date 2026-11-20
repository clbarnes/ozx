# ozx

A CLI for archiving an [OME-Zarr](https://ngff.openmicroscopy.org) hierarchy into a .ozx file [per RFC-9](https://ngff.openmicroscopy.org/rfc/9/index.html).

## Background

`.ozx` files are zip files containing an OME-Zarr hierarchy.
This tool retrieves objects from a range of Zarr storage backends and packs them in to a `.ozx` file.

By default, optimisations recommended by RFC-9 are applied.

## Usage

### Store configuration

`ozx` writes the archive to a local file.
It can read a Zarr hierarchy from a variety of stores (see [Storage backends](#storage-backends)).
Connection details vary wildly by backend.
The simple case (a local directory) simply needs the path,
but more complex cases can be configured using command-line arguments, environment variables, and/ or TOML configuration files.

#### Configuration files

Configuration files are in TOML format, containing a top-level `stores` table whose keys are the user-defined names of stores.
The values are themselves tables with string values; the keys vary by the storage backend.

```toml
[stores.my_store_name]
url = "https://..."
username = "some_username"
# configuration values are always strings
numeric = "1"

[stores.my_other_store]
url = "s3://..."
region = "us-east-1"
```

Multiple configuration files can be given, in which case the configuration is merged,
with later items overriding earlier ones.

### Environment variables

Configuration key-value pairs can be given as environment variables; this is recommended where sensitive credentials are required.
These override configuration file values.

Environment variables are prefixed by `OZX_STORE`.
The key of the configuration pair is upper-cased and appended to the environment variable name after a **double underscore** `__`.
If you are configuring a named store, the name of the store (case insensitive) is added after the prefix, after a single underscore `_`;
otherwise, the configuration is applied to whatever store you're using.
The value is case-sensitive.

Examples:

```sh
# configure a named store
OZX_STORE_MY_STORE_NAME__PASSWORD="potato"
# configure whichever store is in use
OZX_STORE__SSH_KEY="spade"
```

#### Command-line arguments

The STORE argument can be one of

- A store URL (if it has a scheme)
- The name of a store referenced in a configuration file or environment variable
- A path to a local directory

Additional options can be passed with the `-o`/ `--option` argument as `"key=value"` strings.
Many options can be given this way.
These override environment variables and configuration file values.

### Storage backends

The `ozx` tool can, by default, read Zarr hierarchies from:

- Local files
- S3-like services

Additional backends can be compiled in using features of the underlying `opendal` crate; e.g.

```sh
cargo install ozx --features opendal/services-gcs,opendal/services-webdav,opendal/services-azblob
```

Note that the tool depends on the `list` and `list_with_recursive` capabilities.
Services like plain HTTP will not work (but webDAV does).

A full list of services supported by Apache OpenDAL™, and the options needed to configure access to them,
can be found [on the website](https://opendal.apache.org/services/).

### `ozx create`

This needs to list the available stored objects.
By default, it goes directory-by-directory and writes `zarr.json` files first, in breadth-first order,
then the remaining files in arbitrary order.

Some storage backends do not understand the concept of directories,
in which case the full file list may be retrieved many times over this process.
For these stores, use the `--eager` option to fetch the whole file list once and sort it locally.

Use the `--no-sort-metadata` option to write all files in arbitrary order.

### `ozx validate`

This command validates a local file as an OZX archive.

#### Limitations

- Does not check for ZIP64 format extension
- Does not check for multipart zips

## Installation

Choose one of the following.

Install the latest release from crates.io:

```sh
cargo install ozx
ozx --help
```

Install the development head:

```sh
cargo install --git https://github.com/clbarnes/ozx.git
ozx --help
```

Clone the repository for development:

```sh
git clone https://github.com/clbarnes/ozx.git
cd ozx
cargo run -- --help
```

## Attribution

The test data retrieved by `fetch_kingsnake.sh` is as described by [InsightSoftwareConsortium/OMEZarrOpenSciVisDatasets](https://github.com/InsightSoftwareConsortium/OMEZarrOpenSciVisDatasets/tree/main?tab=readme-ov-file#kingsnake).
