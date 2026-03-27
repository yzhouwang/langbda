# Parser Quality Report (Book/Paper-Aligned)

## Scope

This report evaluates whether the current parser behavior matches a small, source-grounded set of English syntax phenomena.

- Runtime under test: movement-aware parser with memoized search.
- Test harness: `cargo test` in this repository.
- Lexicon caveat: coverage is intentionally tiny (`the/an/whose`, `child/apple/room`, `ate/eat/did`, etc.), so many textbook contrasts must be adapted.

## Source Phenomena

1. Do-support and auxiliary inversion in questions.
2. Bare-verb requirement after inserted `do/did`.
3. Wh-dependency behavior (fronting + gap relation).
4. PP-attachment ambiguity.

Reference readings used for these families:

- Catherine Anderson, *Essentials of Linguistics*, 8.11 Do-Support and 8.10 Wh-Movement.
- Colin Phillips (1996), *Order and Structure* (MIT dissertation), especially movement diagnostics (e.g., weak crossover/superiority examples).

## Executable Capability Matrix

| Phenomenon | Source family | Repo fixture | Expected | Observed |
|---|---|---|---:|---:|
| PP attachment ambiguity | classic attachment ambiguity | `the child ate an apple in the room.` | `2` parses | `2` parses |
| Do-support declarative | do-support | `the child did eat an apple.` | `>=1` | `>=1` |
| Yes-no question with aux inversion | do-support/inversion | `did the child eat an apple?` | `>=1` | `>=1` |
| Wh-object question | wh-movement | `whose apple did the child eat?` | `>=1` | `>=1` |
| No-do question | do-support constraint | `the child eat an apple?` | `0` | `0` |
| Lexical verb inversion without aux | do-support constraint | `ate the child an apple?` | `0` | `0` |
| Tensed main verb after `did` (question) | bare-verb requirement | `did the child ate an apple?` | `0` | `0` |
| Tensed main verb after `did` (declarative) | bare-verb requirement | `the child did ate an apple.` | `0` | `0` |
| Malformed wh | wh dependency constraint | `whose did eat an apple?` | `0` | `0` |

## Known Overgeneration (Tracked)

A malformed inversion order is still accepted:

- Fixture: `did child the eat an apple?`
- Current observed behavior: accepted (`>=1` parse)
- Tracked by tests:
  - `english_bad_inversion_order_currently_overgenerated` (active evidence)
  - `english_bad_inversion_order_target_rejected` (ignored target)

Likely cause: current English surface normalization plus permissive feature combinations can still license bad linear orders in edge cases.

## What This Means for Research Use

- Good fit for:
  - controlled experiments on a compact grammar,
  - reproducible parser behavior under explicit test contracts,
  - movement-chain state experiments with memoized search.
- Not yet good as-is for:
  - broad empirical claims about English acceptability,
  - diagnostics requiring richer lexical and structural coverage (e.g., superiority, weak crossover, island families).

## Suggested Next Test Targets (From Phillips 1996)

These are currently out of executable scope with the default tiny lexicon, but should be added after lexicon expansion:

- superiority contrasts,
- weak crossover contrasts,
- island-sensitive wh extraction families.

## Reproducibility

- `cargo test -q` => `27 passed, 0 failed, 5 ignored`
- `cargo clippy --all-targets --all-features -q` => clean
