# cargo-capslock

This is an in-development, experimental tool to analyse Rust projects and
ascertain which [Capslock](https://github.com/google/capslock) capabilities
they require.

This project has been funded by [Alpha-Omega](https://alpha-omega.dev/).

## Status

This is an experimental project, and is not currently written for production
usage by others. It is anticipated that this will eventually be bundled into a
more wide-ranging tool for security-oriented analysis of crates.

## [Code of Conduct][code-of-conduct]

The [Rust Foundation][rust-foundation] has adopted a Code of Conduct that we
expect project participants to adhere to. Please read [the full
text][code-of-conduct] so that you can understand what actions will and will not
be tolerated.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licenses

Like Rust itself, this project is primarily distributed under the terms of both
the MIT license and the Apache License (Version 2.0), with documentation
portions covered by the Creative Commons Attribution 4.0 International license.

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT),
[LICENSE-documentation](LICENSE-documentation), and
[COPYRIGHT](COPYRIGHT) for details.

You can also read more under the Foundation's [intellectual property
policy][ip-policy].

### `llvm-ir-analysis`

The `llvm-ir-analysis` directory contains a patched subtree import of the
[`llvm-ir-analysis` crate][llvm-ir-analysis], which is [MIT licensed and &copy;
Craig Disselkoen](./llvm-ir-analysis/LICENSE).

## Other Policies

You can read about other Rust Foundation policies on the [Rust Foundation
website][policies].

[code-of-conduct]: https://rustfoundation.org/policy/code-of-conduct/
[ip-policy]: https://rustfoundation.org/policy/intellectual-property-policy/
[llvm-ir-analysis]: https://github.com/cdisselkoen/llvm-ir-analysis
[media-guide and trademark]: https://rustfoundation.org/policy/trademark-policy/
[policies]: https://rustfoundation.org/policies-resources/
[rust-foundation]: https://rustfoundation.org/
