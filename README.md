# Langbda

Langbda is an incremental parser that models how a listener receives a linear token stream and builds structured syntactic interpretations.

Current focus is a controlled English grammar with movement-aware parsing behavior inspired by Minimalist Grammar style dependencies.

## What It Can Do Now

- Parse core clause types in a curated grammar:
  - declaratives,
  - PP-attachment ambiguity,
  - do-support declaratives,
  - yes-no questions,
  - wh-object questions.
- Keep ambiguity where intended:
  - `the child ate an apple in the room.` yields exactly `2` parses.
- Track movement-chain lifecycle events in derivation metadata.
- Render parse trees as PNG and export derivation traces as JSON.
- Use memoized search to avoid infinite recursion on cyclic functional rules.

## Quick Start

Build:

```bash
cargo build
```

Run the core fixture batch and export both PNG and JSON:

```bash
cargo run -- --batch core --format both --output-dir assets/examples
```

Parse one sentence:

```bash
cargo run -- --sentence "did the child eat an apple?" --target Sentence --format both --output-dir assets/examples
```

Show CLI help:

```bash
cargo run -- --help
```

## CLI Options

```text
langbda [--sentence "..."] [--target Sentence] [--batch core] [--format png|json|both] [--output-dir DIR] [--no-movement-arrows]
```

- `--sentence`: parse a single input sentence.
- `--target`: parse target category (default `Sentence`).
- `--batch core`: run built-in core fixtures.
- `--format`: artifact format (`png`, `json`, or `both`).
- `--output-dir`: output directory for artifacts.
- `--no-movement-arrows`: disable movement arrows in rendered PNG trees.

## Output Artifacts

For each parse, the CLI writes deterministic files:

- `{sentence_slug}__{target_slug}__parse-XX.png`
- `{sentence_slug}__{target_slug}__parse-XX.json`

JSON includes:

- sentence and target,
- token stream,
- ordered derivation steps (`merge/move/check`),
- movement chain lifecycle events,
- final well-formedness and unresolved chain diagnostics.

## Example Ambiguity

The model captures two interpretations of:

`The child ate an apple in the room.`

![](assets/examples/the-child-ate-an-apple-in-the-room-_tree-1.png "\"in the room\" modifies the TP")
![](assets/examples/the-child-ate-an-apple-in-the-room-_tree-2.png "\"in the room\" modifies the DP")

## Grammar and Resources

- Example lexicon: [assets/lexicons/en.lexicon](assets/lexicons/en.lexicon)
- Capability contract: [docs/capability_matrix.md](docs/capability_matrix.md)
- Quality notes: [docs/quality_report.md](docs/quality_report.md)

## Limits and Scope

- The English lexicon is intentionally small and diagnostic-oriented.
- This is not an open-domain English parser.
- Some question analyses still rely on a constrained normalization bridge in the interpreter, so strict surface-faithful derivation for all constructions is still in progress.
- Formal-language interpretation should be made over this controlled grammar, not broad natural-language coverage.
