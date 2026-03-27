# Capability Matrix

This matrix freezes the parser capability contract for the movement upgrade rollout.

| Construction | Fixture | Label | baseline_now (historical) | target_pending (contract) | current_status |
|---|---|---|---:|---:|---|
| PP attachment ambiguity | `the child ate an apple in the room.` | `Sentence` | `2` | `2` | active (`2`) |
| do-support declarative | `the child did eat an apple.` | `Sentence` | `0` | `>=1` | active (`>=1`) |
| yes-no question | `did the child eat an apple?` | `Sentence` | `0` | `>=1` | active (`>=1`) |
| wh-question | `whose apple did the child eat?` | `Sentence` | `0` | `>=1` | active (`>=1`) |
| MOVED handling | `MOVED(...)` conversion and chain behavior | n/a | flattened behavior documented | explicit chain introduction/discharge | active chain tests |

## Notes

- Historical unsupported tests are kept as ignored tests in the suite for traceability.
- Target tests are active and enforce minimum support (`>=1`) for new constructions.
- PP ambiguity remains exact-count regression (`2` parses).
