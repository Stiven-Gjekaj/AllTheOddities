<div align="center">
  <a href="../README.md"><b>AllTheOddities</b></a>
</div>

# Releases

## The version

The version is the date of the release and a count: `YYYY.MM.N`.

- The first release of a month has `N = 0`.
- Each next release in the same month adds one, for a fix or for a new
  oddity. A new oddity never waits for the next month.
- The tag is `v` and the version, such as `v2026.12.0`.
- The version is also valid semver, so Cargo takes it as it is.

## The codename

Each month that has a release takes the next codename of the list. A month
with no release takes no codename, so no letter is lost. A `.N` release keeps
the codename of its month. The theme is a cabinet of curiosities.

Astrolabe, Barometer, Chronometer, Diorama, Ephemeris, Fossil, Gyroscope,
Hourglass, Inclinometer, Jacquard, Kaleidoscope, Lodestone, Microscope,
Narwhal, Orrery, Planisphere, Quadrant, Reliquary, Sextant, Telescope,
Unicorn, Vellum, Wunderkammer, Xylograph, Yardstick, Zoetrope.

The list is `CODENAMES` in `crates/alltheoddities/src/version.rs`, and the
codename of the build is the `codename!` macro beside it. The release title
is the month and the codename, such as `2026.12 "Astrolabe"`.
`odd --version` prints `odd 2026.12.0 (Astrolabe)`.

## The labels

| Label | What it promises |
| :-- | :-- |
| experimental | Nothing. Flags and output can change in any release. |
| stable | Its flags and exit codes stay the same. A change that breaks them needs a new oddity name, or a deprecation first. |
| deprecated | It still runs, and it prints a warning. It goes away no earlier than the release after the one that marked it. |

Each new oddity starts as experimental. It becomes stable when CI passes on
each host that it supports and the owner ran it on a real computer of each
host. `odd list` shows the label of each oddity.

Each release note has four parts: **New**, **Now stable**, **Fixed** and
**Removed**. Each entry names the oddity.

## Cut a release

1. Set the version in `Cargo.toml` and, for a new month, the codename in
   `version.rs`. Commit that change alone: `chore: set the version of the next release`.
2. Move the "Unreleased" part of `CHANGELOG.md` under the new version.
3. Tag the commit `v<version>`, and push the tag.
4. The release workflow builds the six binaries, writes `SHA256SUMS`, makes
   an attestation of build provenance, and opens a **draft** release. It stops
   if the tag does not match `odd --version`.
5. Read the draft, then publish it. The installers and `odd update` read the
   latest published release.

A run of the release workflow by hand builds and hashes everything, and
publishes nothing. Use it to test the workflow.

## Check a download

Each release has `SHA256SUMS`. The installers and `odd update` check the
binary against it before they run it. By hand:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

```bash
gh attestation verify odd-x86_64-unknown-linux-musl --repo Stiven-Gjekaj/AllTheOddities
```
