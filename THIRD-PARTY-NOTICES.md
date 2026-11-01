# Third-party notices

The [MIT licence](LICENSE) applies to the project-owned source code. The amara
name and logos are not licensed under that code licence.

## Rust dependencies

The native app resolves third-party Rust packages from crates.io; their versions
are recorded in [Cargo.lock](Cargo.lock), and the packages are not vendored in
this source repository. Each dependency has its own licensing terms. A complete
licence and notice inventory for any redistributed binary is a separate release
task; this file does not claim that review has been completed. The
[locked dependency licence inventory](THIRD-PARTY-DEPENDENCIES.md) lists the
declared licences without asserting legal compatibility or platform inclusion.

## Optional font — not distributed here

The native Rust build expects JetBrainsMono Nerd Font Regular. The font binary
is **not included** in this repository. [Font provenance](assets/fonts/SOURCE.txt)
identifies the exact upstream Git blob and SHA-256 used by the optional fetch
script. The included [OFL text](assets/fonts/OFL.txt) contains the JetBrains
Mono font copyright notice and SIL Open Font License 1.1; its presence does not
settle the licensing of all icon glyphs in the patched font.

Nerd Fonts' current [JetBrains Mono source inventory](https://github.com/ryanoasis/nerd-fonts/blob/master/patched-fonts/JetBrainsMono/README.md)
lists embedded icon sets from different projects under different terms,
including an entry marked unlicensed. Consult the upstream inventory, its
[licence audit](https://github.com/ryanoasis/nerd-fonts/blob/master/license-audit.md),
and the permissions applicable to the exact pinned font before use or any
redistribution. The fetch script retrieves the original font from upstream; it
does not grant any rights in that font or its icon sets.
