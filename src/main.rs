mod cognitive;
mod dialect;
mod error;
mod interner;
mod interpreter;
mod lexicon;
mod logger;
mod syntax;
mod tokenizer;
mod trie;

use self::cognitive::{LambdaModel, TreeModel};
use self::dialect::{Dialect, English};
use self::error::{Error, Result};
use self::interpreter::{build_derivation_artifact, follow, interpret};
use self::logger::init_logger;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    Png,
    Json,
    Both,
}

impl OutputFormat {
    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "png" => Ok(Self::Png),
            "json" => Ok(Self::Json),
            "both" => Ok(Self::Both),
            _ => Err(Error::Cli(format!(
                "Unsupported --format value '{raw}'. Use png|json|both"
            ))),
        }
    }

    fn wants_png(self) -> bool {
        matches!(self, Self::Png | Self::Both)
    }

    fn wants_json(self) -> bool {
        matches!(self, Self::Json | Self::Both)
    }
}

#[derive(Debug)]
struct RunConfig {
    sentence: Option<String>,
    target: String,
    batch: String,
    output_dir: String,
    format: OutputFormat,
    show_movement_arrows: bool,
}

fn parse_args() -> Result<RunConfig> {
    let mut sentence = None;
    let mut target = "Sentence".to_string();
    let mut batch = "core".to_string();
    let mut output_dir = "assets/examples".to_string();
    let mut format = OutputFormat::Both;
    let mut show_movement_arrows = true;

    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--sentence" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| Error::Cli("--sentence requires a value".to_string()))?;
                sentence = Some(value.clone());
                i += 1;
            }
            "--target" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| Error::Cli("--target requires a value".to_string()))?;
                target = value.clone();
                i += 1;
            }
            "--batch" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| Error::Cli("--batch requires a value".to_string()))?;
                batch = value.clone();
                i += 1;
            }
            "--output-dir" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| Error::Cli("--output-dir requires a value".to_string()))?;
                output_dir = value.clone();
                i += 1;
            }
            "--format" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| Error::Cli("--format requires a value".to_string()))?;
                format = OutputFormat::parse(value)?;
                i += 1;
            }
            "--no-movement-arrows" => {
                show_movement_arrows = false;
            }
            _ => {
                return Err(Error::Cli(format!(
                    "Unknown argument '{arg}'. Use --help for usage"
                )));
            }
        }
        i += 1;
    }

    Ok(RunConfig {
        sentence,
        target,
        batch,
        output_dir,
        format,
        show_movement_arrows,
    })
}

fn print_help() {
    println!(
        "Usage:\n  langbda [--sentence \"...\"] [--target Sentence] [--batch core] [--format png|json|both] [--output-dir DIR] [--no-movement-arrows]"
    );
}

fn core_fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        ("the child ate an apple in the room.", "Sentence"),
        ("the child ate an apple.", "Sentence"),
        ("the child did eat an apple.", "Sentence"),
        ("did the child eat an apple?", "Sentence"),
        ("whose apple did the child eat?", "Sentence"),
    ]
}

fn sanitize_for_filename(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
}

fn ensure_output_dir(path: &str) -> Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

fn write_json_file(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let serialized = serde_json::to_string_pretty(value)?;
    fs::write(path, serialized)?;
    Ok(())
}

fn main() -> Result<()> {
    init_logger();

    let config = parse_args()?;
    ensure_output_dir(&config.output_dir)?;

    let dialect = English::init();
    let name = dialect.name();

    let fixtures = if let Some(sentence) = config.sentence.clone() {
        vec![(sentence, config.target.clone())]
    } else {
        match config.batch.as_str() {
            "core" => core_fixtures()
                .into_iter()
                .map(|(sentence, target)| (sentence.to_string(), target.to_string()))
                .collect::<Vec<_>>(),
            _ => {
                return Err(Error::Cli(format!(
                    "Unsupported --batch value '{}'. Use 'core' or provide --sentence",
                    config.batch
                )));
            }
        }
    };

    for (sentence, target) in fixtures {
        println!("Interpreting \"{sentence}\" as {target} in {name}");
        let result = interpret::<_, LambdaModel<_>>(&dialect, &sentence, &target)?;
        println!("LANGBDA found {} interpretations.", result.len());

        let sentence_slug = sanitize_for_filename(&sentence);
        let target_slug = sanitize_for_filename(&target);

        for (index, actions) in result.into_iter().enumerate() {
            let parse_index = index + 1;
            let base = format!(
                "{}__{}__parse-{:02}",
                sentence_slug, target_slug, parse_index
            );

            if config.format.wants_png() {
                let mut tree = follow::<_, TreeModel<_>>(&target, actions.clone())?;
                tree.prune().map_err(cognitive::Error::from)?;
                let png_path = Path::new(&config.output_dir).join(format!("{base}.png"));
                tree.to_png_with_arrows(
                    png_path
                        .to_str()
                        .ok_or_else(|| Error::Cli("Invalid output path".to_string()))?
                        .to_string(),
                    config.show_movement_arrows,
                )
                .map_err(cognitive::Error::from)?;
            }

            if config.format.wants_json() {
                let artifact = build_derivation_artifact(&sentence, &target, parse_index, &actions)?;
                let json_path = Path::new(&config.output_dir).join(format!("{base}.json"));
                write_json_file(&json_path, &artifact)?;
            }
        }
    }

    Ok(())
}
