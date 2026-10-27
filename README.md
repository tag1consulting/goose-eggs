# Goose Eggs

Helpful in writing [`Goose`](https://docs.rs/goose) load tests.

[![crates.io](https://img.shields.io/crates/v/goose-eggs.svg)](https://crates.io/crates/goose-eggs)
[![Documentation](https://docs.rs/goose-eggs/badge.svg)](https://docs.rs/goose-eggs)
[![Apache-2.0 licensed](https://img.shields.io/crates/l/goose-eggs.svg)](./LICENSE)
[![CI](https://github.com/tag1consulting/goose-eggs/workflows/CI/badge.svg)](https://github.com/tag1consulting/goose-eggs/actions?query=workflow%3ACI)

## Goose compatibility

Each goose-eggs minor release works with one Goose minor release:

| goose-eggs | Goose |
|---|---|
| 0.7 | 0.19 |
| 0.6 | 0.18 |
| 0.5 | 0.17 |

The `main` branch, 0.7.0-dev, targets Goose `main` (0.19.0-dev) through a git dependency, so it cannot be published until Goose 0.19.0 is released. Use goose-eggs 0.6 from crates.io with Goose 0.18. See [RELEASING.md](RELEASING.md).

## Documentation

- [Goose Eggs](https://docs.rs/goose-eggs)
- [Goose Eggs Umami example](https://github.com/tag1consulting/goose-eggs/tree/main/examples/umami)
- [The Goose book](https://book.goose.rs/)
