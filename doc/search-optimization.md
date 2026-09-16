# Search optimization validation

The search service uses per-volume key cursors and a bounded merge buffer. Only
explicit page requests advance the current query. New text requests replace
pending queries, including repeated text, while QueryId continues to reject late
results. Icon extraction runs on a separate worker with one pending job, at most
100 visible paths and a cache limited to 128 entries / 8 MiB of pixels (60-second
TTL). Text publication does not wait for native icon extraction.

Index snapshots are profile-local under `.search-index`, keyed by volume/root and
exclusion configuration. They use streaming atomic replacement, format validation
and a SHA-256 checksum. The checksum detects corruption; it is not a signature or
trust boundary. Existing temporary `.fd` files are neither imported nor deleted.
Windows snapshots preserve the journal identity and continuation USN, validate
the available journal range, and checkpoint changes before release. A missing,
invalid or expired snapshot triggers rebuilding. macOS still scans on startup:
the portable watcher has no persisted event cursor to cover process downtime.

MFT enumeration includes the complete USN range while retaining the journal
position captured before enumeration for subsequent replay. This avoids filtering
out files whose contents change during the scan. The native filtering and journal
continuation rules are documented by Microsoft in
[MFT_ENUM_DATA_V0](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-mft_enum_data_v0)
and [READ_USN_JOURNAL_DATA_V0](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-read_usn_journal_data_v0).

## Reproducible synthetic measurement

Run in isolation on Windows, using the pinned toolchain:

```powershell
cargo test -p rotor-searcher --release --locked file_data::volume::ntfs_file_map::release_tests::measure_ntfs_search_workload -- --ignored --exact --nocapture --test-threads=1
```

The fixture has 250,000 files and one root; one eighth of filenames contain
Chinese text. It performs no volume enumeration, icon extraction or real-profile
access. Cache reload follows a save, so the data is normally in the OS page cache.
Memory is the test process's Private Commit increase during index construction,
not total application RSS or a claim about operating-system memory reclamation.

On the Windows development machine, 2026-09-16, three separate executions after
one warm-up produced these medians:

| Metric | Before compact names / ASCII fast path | After |
| --- | ---: | ---: |
| Build synthetic index | 111.76 ms | 64.41 ms |
| Reload verified snapshot | 114.37 ms | 61.69 ms |
| Private Commit increase | 59,506,688 bytes | 54,558,720 bytes |
| NTFS FileView size | 56 bytes | 48 bytes |

The baseline is `7146f68` with only the measurement test added. These measurements
compare the name-storage and preparation optimization, not the entire series
against the original application. Timings vary with hardware, filesystem cache
and system load. They do not establish native cold-start, real-volume search,
macOS runtime, visual or installation acceptance.

Portable directory updates coalesce parent/child notifications and remove all
changed subtrees in one index traversal per batch. Snapshot serialization drops
unreferenced directory nodes so repeated renames/deletions do not accumulate
obsolete directory storage across releases and reloads.

Regular tests cover cross-volume ordering and duplicate prevention, explicit
paging, index release/reload, Unicode/wildcard/pinyin matching, corrupted and
truncated snapshots, failed atomic replacement, journal identity/range validation,
bounded queues, cancellation registration races and stale icon rejection.
Portable directory-event logic is also compiled and exercised on Windows using
isolated temporary directories; this does not replace macOS FSEvents testing.
