# Releasing goose-eggs

## The version rule

Each goose-eggs minor release targets exactly one Goose minor release:

| goose-eggs | Goose |
|---|---|
| 0.5 | 0.17 |
| 0.6 | 0.18 |
| 0.7 | 0.19 |

A goose-eggs patch release (0.7.1, 0.7.2) fixes or extends goose-eggs and keeps the same Goose minor.

While Goose `main` is a `-dev` version, goose-eggs `main` depends on it by git revision:

```toml
goose = { git = "https://github.com/tag1consulting/goose", rev = "<sha>", default-features = false }
```

A `[patch.crates-io]` override does not work here: a `0.19.0-dev` version does not satisfy `^0.19`. A git dependency cannot be published, so `cargo publish` refuses the crate until the dependency is swapped for a crates.io version.

## Release day checklist: 0.7.0 with Goose 0.19.0

Run these in order, on one branch off `main`:

1. Goose 0.19.0 is tagged and published on crates.io.
2. In `Cargo.toml`, replace the git dependency with `goose = { version = "0.19", default-features = false }`.
3. In `Cargo.toml`, set `version = "0.7.0"`.
4. In `CHANGELOG.md`, rename `## 0.7.0-dev` to `## 0.7.0 <Month D, YYYY>`, the release date.
5. In `README.md`, replace the paragraph under the Goose compatibility table, which says `main` cannot be published, with one saying this release is on crates.io.
6. Run `cargo publish --dry-run`; it must pass.
7. Push, open the pull request, and wait for CI to be green on that exact commit; merge it.
8. Tag the merge commit `0.7.0` and push the tag.
9. Run `cargo publish`.
10. Bump `Cargo.toml` to `version = "0.7.1-dev"` and open `## 0.7.1-dev` at the top of `CHANGELOG.md`.
11. The same day, open a Goose pull request that updates the Goose Book's goose-eggs lines, which say goose-eggs is not yet released for 0.19: `src/docs/goose-book/src/getting-started/validation.md` and `src/docs/goose-book/src/getting-started/upgrading.md`.

## The next cycle

When Goose `main` moves to `0.20.0-dev`, goose-eggs `main` follows: switch `goose` back to a git dependency pinned to a Goose `main` revision, set `version = "0.8.0-dev"`, port to the new Goose, and open `## 0.8.0-dev` in `CHANGELOG.md`. In `README.md`, the paragraph under the Goose compatibility table goes back to saying `main` targets Goose `main` through a git dependency, so it cannot be published until the next Goose minor release. Bump the pinned revision when goose-eggs needs a newer Goose change. Release 0.8.0 with Goose 0.20.0 by the checklist above, with the versions moved up by one.
