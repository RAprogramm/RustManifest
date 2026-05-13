# rustmanifest-rules-core

Default rules pack for `rustmanifest`. Rules are parsed at build time from the English methodology markdown under `code-review-methodology/en/`, `README.md`, and `STRUCTURE.md`, and bundled with pass/fail fixtures.

This pack is versioned independently from the engine — see `PACK_VERSION`. The engine pins a SemVer range and refuses to load incompatible packs.

Phase 0 ships only the pack identifier and version constants; rule definitions land in Phase 1.
