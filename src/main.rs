use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufReader, Seek, Write};
use std::path::PathBuf;

use clap::Parser;
use xmltree::{Element, EmitterConfig, XMLNode};
use zip::read::ZipFile;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Prefix for output filename
    #[clap(short, long, default_value = "_licensed")]
    suffix: String,

    /// File containing the metadata to be added to the 3MF
    #[clap(short, long, default_value = "metadata.xml")]
    metadata: OsString,

    /// Force overwrite of existing files
    #[clap(short, long)]
    force: bool,

    /// Input file(s)
    #[clap(forbid_empty_values = true, required = true)]
    input: Vec<OsString>,
}

fn update_xml_and_copy<W>(mut file: ZipFile, metadata: &Element, output: &mut ZipWriter<W>)
where
    W: Write + Seek,
{
    // these shouldn't fail, because we use enclosed_name() to determine
    // whether to get here. And enclosed_name returning Some means
    // to_str() will work.
    let file_name: String = file.enclosed_name().unwrap().to_str().unwrap().to_string();

    let mut xml = Element::parse(&mut file).unwrap();
    if xml.children.iter().any(|child| match child {
        XMLNode::Element(element) => element.name == "metadata",
        _ => false,
    }) {
        println!("Metadata found in file {}, not adding any", file_name);
        // do a raw copy instead
        output
            .raw_copy_file(file)
            .unwrap_or_else(|_| panic!("writing raw copy failed"));
        return;
    }
    // clone metadata's children to a new Vec
    let mut metadata = metadata.children.to_vec();
    // metadata needs to be at the start, so we append the existing children to
    // the metadata Vec, leaving xml.children empty. And then append the whole
    // thing back.
    metadata.append(&mut xml.children);
    xml.children.append(&mut metadata);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9));
    output.start_file(&file_name, options).unwrap();
    let config = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("\t")
        .line_separator("\n");
    xml.write_with_config(output, config).unwrap();
    println!("Added metadata to file {}", file_name);
}

fn main() {
    let args = Args::parse();
    //println!("{:?}", args);

    // read metadata file
    let metadata = BufReader::new(File::open(&args.metadata).unwrap());
    let metadata =
        Element::parse(metadata).unwrap_or_else(|_| panic!("Could not parse metadata file"));
    if metadata.name != "v1" {
        println!("Metadata file is not a v1 file");
        std::process::exit(1);
    }
    if metadata.children.iter().any(|child| match child {
        XMLNode::Element(element) => element.name != "metadata",
        _ => true,
    }) {
        println!("Metadata file contains XML elements other than v1 and its metadata children");
        std::process::exit(1);
    }
    if !metadata.children.iter().any(|child| match child {
        XMLNode::Element(element) => element.name == "metadata",
        _ => false,
    }) {
        println!("Metadata file has no metadata elements");
        std::process::exit(1);
    }

    #[cfg(windows)]
    let input_file_names = args
        .input
        .into_iter()
        .flat_map(|file_name| {
            if let Some(file_name) = file_name.to_str() {
                glob::glob(file_name)
                    .unwrap_or_else(|_| panic!("Could not glob {}", file_name))
                    .map(|path| path.unwrap())
                    .collect::<Vec<_>>()
            } else {
                println!("Using file {} as is", file_name.to_string_lossy());
                vec![PathBuf::from(file_name)]
            }
        })
        .collect::<Vec<_>>();

    #[cfg(not(windows))]
    let input_file_names = args
        .input
        .into_iter()
        .map(|file_name| PathBuf::from(file_name))
        .collect::<Vec<_>>();

    println!("Number of input files: {}", input_file_names.len());
    // loop over input files, exit with an error if any input
    // file starts with our prefix, or don't exist.
    for input_path in &input_file_names {
        println!("Processing {}", input_path.to_string_lossy());
        if !input_path.exists() {
            println!("{} does not exist", input_path.to_string_lossy());
            std::process::exit(1);
        }
        if !input_path.is_file() {
            println!("{} is not a file", input_path.to_string_lossy());
            std::process::exit(1);
        }
        let output_file_name;
        if let (Some(stem), extension) = (input_path.file_stem(), input_path.extension()) {
            if stem.to_string_lossy().ends_with(&args.suffix) {
                println!(
                    "Skipping {}, because it already ends with suffix, exiting",
                    input_path.display()
                );
                continue;
            }
            let mut name = stem.to_os_string();
            name.push(OsStr::new(&args.suffix));
            if let Some(extension) = extension {
                name.push(OsString::from("."));
                name.push(extension);
            }
            output_file_name = Some(name);
        } else {
            panic!("Could not get file stem from {}", input_path.display());
        }
        // Shouldn't fail because of the panic above
        let output_file_name = output_file_name.unwrap();
        let output_path = input_path.with_file_name(output_file_name);

        if output_path.exists() && !args.force {
            println!(
                "{} already exists, use -f or --force to ignore",
                output_path.to_string_lossy()
            );
            std::process::exit(1);
        }

        // open input file
        let input = File::open(input_path).unwrap_or_else(|_| {
            panic!("Failed to open input file {}", input_path.to_string_lossy())
        });
        let input = BufReader::new(input);
        let mut input = ZipArchive::new(input).unwrap();
        // open output file
        let output = File::create(&output_path).unwrap_or_else(|_| {
            panic!(
                "Failed to open output file {}",
                output_path.to_string_lossy()
            )
        });
        let mut output = ZipWriter::new(output);
        // copy all files from input to output
        for file_number in 0..input.len() {
            let file = input
                .by_index(file_number)
                .expect("failure reading from ZIP archive");
            match file.enclosed_name() {
                Some(path) if path.extension() == Some(OsStr::new("model")) => {
                    update_xml_and_copy(file, &metadata, &mut output)
                }
                _ => output.raw_copy_file(file).expect("writing raw copy failed"),
            };
        }
        output
            .finish()
            .expect("failed to finish writing ZIP archive");
    }
}
