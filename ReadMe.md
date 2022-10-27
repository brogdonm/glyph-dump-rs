# Glyph Dump Tool

A tool written in Rust for dumping glyphs from a font.

## Getting Started

Basic usage:

```cmd
cargo run -- --font-file test.otf
```

Using a different scaling factor:

```cmd
cargo run -- --font-file test.otf --scale-factor 512.0
```

Changing the output color (in this case green):

```cmd
cargo run -- --font-file test.otf --color-red 0 --color-green 255 --color-blue 0
```

For more usage see: `cargo run -- --help`
