use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{stdout, BufReader, Seek, Write};
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use xmltree::{Element, EmitterConfig, XMLNode};
use zip::read::ZipFile;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Subcommands,
}

#[derive(Subcommand, Debug)]
enum Subcommands {
    /// Add metadata to 3MF files
    Add(Add),
    /// Show metadata in 3MF files
    Show(Show),
}

#[derive(Args, Debug)]
struct Add {
    /// Prefix for output filename
    #[clap(short, long, default_value = "_licensed")]
    suffix: String,

    /// File containing the metadata to be added to the 3MF
    #[clap(short, long, default_value = "metadata.xml")]
    metadata: OsString,

    /// Whether to keep existing metadata tags when one of the same
    /// name is found in the metadata file.
    #[clap(short, long)]
    keep_existing: bool,

    /// Set Title to filename
    #[clap(short, long)]
    title: bool,

    /// Force overwrite of existing files
    #[clap(short, long)]
    force: bool,

    /// Input file(s)
    #[clap(forbid_empty_values = true, required = true)]
    input_files: Vec<OsString>,

    // output file is just used internally for add commands
    #[clap(skip)]
    output_path: Option<PathBuf>,

    // title_value is just used internally for add commands
    #[clap(skip)]
    title_value: Option<String>,

    // metadata read from file, also internal only
    #[clap(skip)]
    metadata_xml: Option<Element>,
}

#[derive(Args, Debug)]
struct Show {
    /// Input file(s)
    #[clap(forbid_empty_values = true, required = true)]
    input_files: Vec<OsString>,
}

fn add_metadata_to_hashmap(metadata_map: &mut HashMap<String, XMLNode>, metadata: &Element) {
    for child in metadata.children.iter() {
        match child {
            XMLNode::Element(element) => {
                metadata_map.insert(
                    element.attributes["name"].clone(),
                    XMLNode::Element(element.clone()),
                );
            }
            _ => panic!("metadata element is not an element"),
        }
    }
}

fn update_xml_and_copy<W>(
    mut file: ZipFile,
    metadata: &Element,
    output: &mut ZipWriter<W>,
    keep_existing: bool,
    title: &Option<String>,
) -> bool
where
    W: Write + Seek,
{
    // these shouldn't fail, because we use enclosed_name() to determine
    // whether to get here. And enclosed_name returning Some means
    // to_str() will work.
    let file_name: String = file.enclosed_name().unwrap().to_str().unwrap().to_string();

    let mut xml = Element::parse(&mut file).unwrap();

    // move xml's children to temporary vec.
    let mut children: Vec<XMLNode> = Vec::new();
    children.append(&mut xml.children);

    // add all metadata elements in xml to a hashmap, then add the metadata
    // elements as well, overwriting any existing metadata. Or vice versa
    // if keep_existing is true.
    let mut metadata_map: HashMap<String, XMLNode> = HashMap::new();
    // if we keep the existing metadata, add the new metadata to the map first.
    if keep_existing {
        add_metadata_to_hashmap(&mut metadata_map, metadata)
    }
    // put metadata children into the hashmap, add everything else to a vec
    // to be added to the xml after the metadata.
    let other_elements: Vec<_> = children
        .into_iter()
        .filter_map(|child| match child {
            XMLNode::Element(element) if element.name == "metadata" => {
                metadata_map.insert(
                    element.attributes["name"].clone(),
                    XMLNode::Element(element),
                );
                None
            }
            _ => Some(child),
        })
        .collect();
    // if we don't keep the existing metadata, add the new metadata to the map last.
    if !keep_existing {
        add_metadata_to_hashmap(&mut metadata_map, metadata)
    }
    // Set title if requested
    if let Some(title) = title {
        eprintln!("setting title to {}", title);
        // make a new element with the title
        let mut title_element = Element::new("metadata");
        title_element.attributes.insert("name".to_string(), "Title".to_string());
        title_element.children.push(XMLNode::Text(title.clone()));
        metadata_map.insert("Title".to_string(), XMLNode::Element(title_element));
    }

    // now add the hashmap to the xml.
    for node in metadata_map.into_values() {
        xml.children.push(node);
    }
    // and add the other elements to the xml.
    xml.children.extend(other_elements);

    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9));
    output.start_file(&file_name, options).unwrap();
    let config = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("\t")
        .line_separator("\n");
    xml.write_with_config(output, config).unwrap();
    eprintln!("Added metadata to file {}", file_name);
    true
}

fn show_metadata(file: ZipFile) {
    // Like above, should not fail
    let file_name: String = file.enclosed_name().unwrap().to_str().unwrap().to_string();

    let xml = Element::parse(file).unwrap();
    let metadata = xml
        .children
        .into_iter()
        .filter_map(|child| match child {
            XMLNode::Element(mut element) => {
                if element.name == "metadata" {
                    element.namespace = None;
                    element.namespaces = None;
                    Some(element)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if metadata.is_empty() {
        eprintln!("No metadata found in file {}", file_name);
    } else {
        eprintln!("Metadata found in file {}:", file_name);
        let config = EmitterConfig::new()
            .perform_indent(true)
            .indent_string("\t")
            .line_separator("\n")
            .write_document_declaration(false);
        for element in metadata {
            element.write_with_config(stdout(), config.clone()).unwrap();
            println!();
        }
    }
}

fn main() {
    let mut cli = Cli::parse();
    //eprintln!("{:?}", args);

    if let Subcommands::Add(ref mut add) = cli.subcommand {
        // read metadata file
        let metadata = BufReader::new(File::open(&add.metadata).unwrap());
        let metadata =
            Element::parse(metadata).unwrap_or_else(|_| panic!("Could not parse metadata file"));
        if metadata.name != "v1" {
            eprintln!("Metadata file is not a v1 file");
            std::process::exit(1);
        }
        if metadata.children.iter().any(|child| match child {
            XMLNode::Element(element) => element.name != "metadata",
            _ => true,
        }) {
            eprintln!(
                "Metadata file contains XML elements other than v1 and its metadata children"
            );
            std::process::exit(1);
        }
        if !metadata.children.iter().any(|child| match child {
            XMLNode::Element(element) => element.name == "metadata",
            _ => false,
        }) {
            eprintln!("Metadata file has no metadata elements");
            std::process::exit(1);
        }
        add.metadata_xml = Some(metadata);
    }

    let input_files = match cli.subcommand {
        Subcommands::Add(ref add) => &add.input_files,
        Subcommands::Show(ref show) => &show.input_files,
    };

    #[cfg(windows)]
    let expanded_input_files = input_files
        .into_iter()
        .flat_map(|file_name| {
            if let Some(file_name) = file_name.to_str() {
                glob::glob(file_name)
                    .unwrap_or_else(|_| panic!("Could not glob {}", file_name))
                    .map(|path| path.unwrap())
                    .collect::<Vec<_>>()
            } else {
                vec![PathBuf::from(file_name)]
            }
        })
        .collect::<Vec<_>>();

    #[cfg(not(windows))]
    let input_file_names = input_files
        .into_iter()
        .map(|file_name| PathBuf::from(file_name))
        .collect::<Vec<_>>();

    eprintln!("Number of input files: {}", expanded_input_files.len());
    // loop over input files, exit with an error if any input
    // file starts with our prefix, or don't exist.
    for input_path in &expanded_input_files {
        eprintln!("Processing {}", input_path.to_string_lossy());
        if !input_path.exists() {
            eprintln!("{} does not exist", input_path.to_string_lossy());
            std::process::exit(1);
        }
        if !input_path.is_file() {
            eprintln!("{} is not a file", input_path.to_string_lossy());
            std::process::exit(1);
        }
        if let Subcommands::Add(ref mut add) = cli.subcommand {
            let output_file_name;
            if let (Some(stem), extension) = (input_path.file_stem(), input_path.extension()) {
                if stem.to_string_lossy().ends_with(&add.suffix) {
                    eprintln!(
                        "Skipping {}, because it already ends with suffix, exiting",
                        input_path.display()
                    );
                    continue;
                }
                let mut name = stem.to_os_string();
                name.push(OsStr::new(&add.suffix));
                if add.title {
                    add.title_value = Some(name.to_string_lossy().to_string());
                }
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

            if output_path.exists() && !add.force {
                eprintln!(
                    "{} already exists, use -f or --force to ignore",
                    output_path.to_string_lossy()
                );
                std::process::exit(1);
            }
            add.output_path = Some(output_path);
        }
        // open input file
        let input = File::open(input_path).unwrap_or_else(|_| {
            panic!("Failed to open input file {}", input_path.to_string_lossy())
        });
        let input = BufReader::new(input);
        let mut input = ZipArchive::new(input).unwrap();

        match cli.subcommand {
            Subcommands::Add(ref add) => {
                // open output file
                let output_path = add.output_path.as_ref().unwrap();
                let output = File::create(output_path).unwrap_or_else(|_| {
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
                    let mut updated = false;
                    match file.enclosed_name() {
                        Some(path) if path.extension() == Some(OsStr::new("model")) => {
                            updated = update_xml_and_copy(
                                file,
                                add.metadata_xml.as_ref().unwrap(),
                                &mut output,
                                add.keep_existing,
                                &add.title_value,
                            )
                        }
                        _ => {
                            drop(file);
                        }
                    }

                    if !updated {
                        let file = input
                            .by_index_raw(file_number)
                            .expect("failure reading from ZIP archive");
                        output.raw_copy_file(file).expect("writing raw copy failed");
                    }
                }
                output
                    .finish()
                    .expect("failed to finish writing ZIP archive");
            }
            Subcommands::Show(_) => {
                for file_number in 0..input.len() {
                    let file = input
                        .by_index(file_number)
                        .expect("failure reading from ZIP archive");
                    match file.enclosed_name() {
                        Some(path) if path.extension() == Some(OsStr::new("model")) => {
                            show_metadata(file)
                        }
                        _ => (),
                    };
                }
            }
        }
    }
}
