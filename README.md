# metadata_3mf

Small utility to add or show metadata to/in 3MF files.

# Downloading

You can always download the latest release at https://github.com/docwilco/metadata_3mf/releases

# Building

Should be as easy as [installing Rust](https://www.rust-lang.org/tools/install) and running `cargo build --release`, you'll find the binary in the `target/release` directory.

# Usage

```
> metadata_3mf help
metadata_3mf 0.3.0

USAGE:
    metadata_3mf.exe <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    add     Add metadata to 3MF files
    help    Print this message or the help of the given subcommand(s)
    show    Show metadata in 3MF files
```

```
> metadata_3mf help add
metadata_3mf.exe-add 0.3.0
Add metadata to 3MF files

USAGE:
    metadata_3mf.exe add [OPTIONS] <INPUT_FILES>...

ARGS:
    <INPUT_FILES>...    Input file(s)

OPTIONS:
    -f, --force                  Force overwrite of existing files
    -h, --help                   Print help information
    -k, --keep-existing          Whether to keep existing metadata tags when one of the same name is
                                 found in the metadata file
    -m, --metadata <METADATA>    File containing the metadata to be added to the 3MF [default:
                                 metadata.xml]
    -s, --suffix <SUFFIX>        Prefix for output filename [default: _licensed]
    -t, --title                  Set Title to filename
    -V, --version                Print version information
```

```
> metadata_3mf help show
metadata_3mf.exe-show 0.3.0
Show metadata in 3MF files

USAGE:
    metadata_3mf.exe show <INPUT_FILES>...

ARGS:
    <INPUT_FILES>...    Input file(s)

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information
```

