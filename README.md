# ozx

A toy CLI for archving an [OME-Zarr](https://ngff.openmicroscopy.org) hierarchy into a .ozx file [per RFC-9](https://ngff.openmicroscopy.org/rfc/9/index.html).

## Limitations

This was originally intended as an exploration of the format rather than a production tool,
and is currently limited to local OME-Zarr stores and runs on a single thread.

## Attribution

The test data retrieved by `fetch_kingsnake.sh` is as described by [InsightSoftwareConsortium/OMEZarrOpenSciVisDatasets](https://github.com/InsightSoftwareConsortium/OMEZarrOpenSciVisDatasets/tree/main?tab=readme-ov-file#kingsnake).
