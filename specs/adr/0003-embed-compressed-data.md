# Superseded: commit and embed every reference-data resource

**Status:** Superseded by ADR 0005.

The original decision was to store every required reference database compressed in Git and embed it in the Rust binary. Although the approximately 68 MiB collection fit GitHub's file limits, embedding governed third-party data would make the combined repository and binary subject to that data's distribution restrictions. The project instead keeps the code MIT-licensed and downloads the required reference data from public upstream sources into each user's local cache.
