# Glyph Dump Tool

A tool written in Rust for dumping glyphs from a font.

## Getting Started

Basic usage:

```cmd
cargo run -- --font-file test.otf
```

Using a different image size:

```cmd
cargo run -- --font-file test.otf --img-size 512
```

Changing the output color (in this case green):

```cmd
cargo run -- --font-file test.otf --color #00ff00
```

Specify the range:

```cmd
cargo run -- --font-file test.otf --unicode-range 0x00..0xffff
```

> NOTE: The values can be in the form of `0x####`, `U+####` or `####`.

For more usage see: `cargo run -- --help`

## Optional Features

By default the tool is built with parallel processing, controlled by `feature = "parallel"`.  To build without the parallel processing:

```cmd
cargo build --no-default-features
```
