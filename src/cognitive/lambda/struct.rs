use super::super::CognitiveModel;
use super::error::{Error, Result};
use super::node::Node;
use super::valid_entry::ValidEntry;
use crate::lexicon::LexiconEntry;
use crate::syntax::{FeatureSet, SyntaxValue};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChainStatus {
    Pending,
    Discharged,
}

#[derive(Debug, Clone)]
struct MovementChain<K> {
    id: usize,
    requirement: FeatureSet<K>,
    introduced_at: usize,
    discharged_at: Option<usize>,
    status: ChainStatus,
}

#[derive(Debug, Clone)]
enum DerivationOp<K> {
    MergeToken { token: K },
    MergeEntry { entry: LexiconEntry<K> },
    MoveIntroduce { chain_id: usize, requirement: FeatureSet<K> },
    CheckDischarge {
        chain_id: usize,
        requirement: FeatureSet<K>,
        provided: FeatureSet<K>,
    },
}

#[derive(Debug, Clone)]
struct DerivationEvent<K> {
    step: usize,
    op: DerivationOp<K>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivationStepRecord {
    pub step: usize,
    pub operation: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLifecycleRecord {
    pub chain_id: usize,
    pub requirement: String,
    pub introduced_at: usize,
    pub discharged_at: Option<usize>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct LambdaModel<K> {
    expects: Vec<Node<K>>,
    chains: Vec<MovementChain<K>>,
    next_chain_id: usize,
    step_counter: usize,
    derivation_events: Vec<DerivationEvent<K>>,
    trace_enabled: bool,
}

impl<K: Clone + Ord> LambdaModel<K> {
    fn new() -> Self {
        Self {
            expects: Vec::new(),
            chains: Vec::new(),
            next_chain_id: 1,
            step_counter: 0,
            derivation_events: Vec::new(),
            trace_enabled: false,
        }
    }
    fn is_empty(&self) -> bool {
        self.expects.is_empty()
    }
    fn push(&mut self, expect: Node<K>) {
        self.expects.push(expect);
    }
    fn push_lambda(&mut self, from: SyntaxValue<K>, to: Node<K>) {
        self.push(Node::Lambda {
            from,
            to: Box::new(to),
        });
    }
    fn push_lambda_features(&mut self, from: FeatureSet<K>, to: FeatureSet<K>) -> Result<bool> {
        self.discharge_pending_chain_with(&from);
        if to.is_subset(&from) {
            self.possibly_project(&from)?;
            Ok(false)
        } else {
            self.push_lambda(SyntaxValue::from(from), Node::from(to));
            Ok(true)
        }
    }
    fn push_projection(&mut self, ignore: FeatureSet<K>) {
        self.push(Node::Projection { ignore });
    }
    fn peek(&self) -> Option<&Node<K>> {
        self.expects.last()
    }
    fn peek_mut(&mut self) -> Option<&mut Node<K>> {
        self.expects.last_mut()
    }
    fn pop(&mut self) -> Option<Node<K>> {
        self.expects.pop()
    }
    fn pop_node(&mut self) -> Result<Node<K>> {
        self.pop().ok_or(Error::NoExpectation)
    }
    fn begin_step(&mut self) {
        self.step_counter += 1;
    }
    fn log_event(&mut self, op: DerivationOp<K>) {
        if self.trace_enabled {
            self.derivation_events.push(DerivationEvent {
                step: self.step_counter,
                op,
            });
        }
    }
    fn add_pending_chain(&mut self, needed: FeatureSet<K>) -> usize {
        let chain_id = self.next_chain_id;
        self.next_chain_id += 1;

        self.log_event(DerivationOp::MoveIntroduce {
            chain_id,
            requirement: needed.clone(),
        });
        self.chains.push(MovementChain {
            id: chain_id,
            requirement: needed,
            introduced_at: self.step_counter,
            discharged_at: None,
            status: ChainStatus::Pending,
        });
        chain_id
    }
    fn discharge_pending_chain_with(&mut self, provided: &FeatureSet<K>) -> Option<usize> {
        let chain_index = self
            .chains
            .iter()
            .enumerate()
            .filter(|(_, chain)| chain.status == ChainStatus::Pending)
            .find(|(_, chain)| {
                chain.requirement.is_feature_compatible_subset(provided)
                    || chain.requirement.overlaps_category(provided)
            })
            .map(|(index, _)| index);

        if let Some(index) = chain_index {
            let chain = &mut self.chains[index];
            chain.status = ChainStatus::Discharged;
            chain.discharged_at = Some(self.step_counter);
            let chain_id = chain.id;
            let requirement = chain.requirement.clone();
            self.log_event(DerivationOp::CheckDischarge {
                chain_id,
                requirement,
                provided: provided.clone(),
            });
            return Some(chain_id);
        }

        None
    }
    #[allow(dead_code)]
    fn count_pending_chains(&self) -> usize {
        self.chains
            .iter()
            .filter(|chain| chain.status == ChainStatus::Pending)
            .count()
    }
    fn pending_chain_requirements(&self) -> Vec<&FeatureSet<K>> {
        self.chains
            .iter()
            .filter(|chain| chain.status == ChainStatus::Pending)
            .map(|chain| &chain.requirement)
            .collect()
    }
    fn possibly_project(&mut self, from: &FeatureSet<K>) -> Result<()> {
        if let Some(Node::Projection { .. }) = self.peek() {
            let ignore = match self.pop() {
                Some(Node::Projection { ignore }) => ignore,
                _ => unreachable!("Already checked that expect stack top is Projection"),
            };

            if let Some(node) = self.peek_mut() {
                if let Some(onto) = node.get_features_left_mut() {
                    FeatureSet::project(from, onto, &ignore)?;
                }
            }
        }
        Ok(())
    }

    fn valid_entry_to_node(&mut self, entry: ValidEntry<K>) -> Result<Node<K>> {
        match entry {
            ValidEntry::Features(fs) => {
                self.discharge_pending_chain_with(&fs);
                Ok(Node::from(fs))
            }
            ValidEntry::Moved { from } => {
                self.add_pending_chain(from.clone());
                Ok(Node::from(from))
            }
            ValidEntry::Lambda {
                from,
                to,
                project: _,
            } => match *from {
                ValidEntry::Features(from_fs) => {
                    self.discharge_pending_chain_with(&from_fs);
                    Ok(Node::Lambda {
                        from: SyntaxValue::Features(from_fs),
                        to: Box::new(Node::from(to)),
                    })
                }
                // MOVED(X) > Y is interpreted as a gap requirement on X,
                // while still keeping a local lambda expectation.
                ValidEntry::Moved { from: moved_need } => {
                    self.add_pending_chain(moved_need.clone());
                    Ok(Node::Lambda {
                        from: SyntaxValue::Features(moved_need),
                        to: Box::new(Node::from(to)),
                    })
                }
                ValidEntry::Lambda { .. } => Err(Error::TypeConversion),
            },
        }
    }

    #[cfg(test)]
    pub fn pending_chain_count(&self) -> usize {
        self.count_pending_chains()
    }
}

impl<K: Clone + Ord> LambdaModel<K> {
    pub fn enable_trace(&mut self) {
        self.trace_enabled = true;
    }
}

impl<K: Clone + Ord + Display> LambdaModel<K> {
    pub fn derivation_step_records(&self) -> Vec<DerivationStepRecord> {
        self.derivation_events
            .iter()
            .map(|event| {
                let (operation, detail) = match &event.op {
                    DerivationOp::MergeToken { token } => {
                        ("merge".to_string(), format!("token {}", token))
                    }
                    DerivationOp::MergeEntry { entry } => {
                        ("merge".to_string(), format!("entry {}", entry))
                    }
                    DerivationOp::MoveIntroduce {
                        chain_id,
                        requirement,
                    } => (
                        "move".to_string(),
                        format!("CH{} introduced for {}", chain_id, requirement),
                    ),
                    DerivationOp::CheckDischarge {
                        chain_id,
                        requirement,
                        provided,
                    } => (
                        "check".to_string(),
                        format!("CH{}: {} <= {}", chain_id, requirement, provided),
                    ),
                };
                DerivationStepRecord {
                    step: event.step,
                    operation,
                    detail,
                }
            })
            .collect()
    }

    pub fn chain_lifecycle_records(&self) -> Vec<ChainLifecycleRecord> {
        let mut records = self
            .chains
            .iter()
            .map(|chain| ChainLifecycleRecord {
                chain_id: chain.id,
                requirement: format!("{}", chain.requirement),
                introduced_at: chain.introduced_at,
                discharged_at: chain.discharged_at,
                status: match chain.status {
                    ChainStatus::Pending => "pending".to_string(),
                    ChainStatus::Discharged => "discharged".to_string(),
                },
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.chain_id);
        records
    }

    pub fn unresolved_chain_requirements(&self) -> Vec<String> {
        let mut unresolved = self
            .chains
            .iter()
            .filter(|chain| chain.status == ChainStatus::Pending)
            .map(|chain| format!("{}", chain.requirement))
            .collect::<Vec<_>>();
        unresolved.sort();
        unresolved
    }
}

impl<K: Clone + Ord> Default for LambdaModel<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// interface with lexicon
mod lexicon {
    use super::*;
    use crate::lexicon::LexiconEntry;
    impl<K: Clone + Ord> LambdaModel<K> {
        pub fn push_lexicon_lambda(&mut self, from: ValidEntry<K>, to: Node<K>) -> Result<bool> {
            // do not add projection if insertion fails due to being subset

            match (from, to) {
                (ValidEntry::Features(from), Node::Value { value: to }) => match to {
                    SyntaxValue::Features(to) => self.push_lambda_features(from, to),
                    _ => Err(Error::LambdaToMustBeFeatures),
                },
                (ValidEntry::Features(from), to @ Node::Lambda { .. }) => {
                    self.discharge_pending_chain_with(&from);
                    let from = SyntaxValue::from(from);
                    self.push_lambda(from, to);
                    Ok(true)
                }
                (
                    ValidEntry::Moved { from },
                    Node::Value {
                        value: SyntaxValue::Features(to),
                    },
                ) => {
                    self.add_pending_chain(from.clone());
                    if to.is_subset(&from) {
                        self.possibly_project(&from)?;
                        Ok(false)
                    } else {
                        self.push_lambda(SyntaxValue::from(from), Node::from(to));
                        Ok(true)
                    }
                }
                (ValidEntry::Moved { from }, to @ Node::Lambda { .. }) => {
                    self.add_pending_chain(from.clone());
                    let from = SyntaxValue::from(from);
                    self.push_lambda(from, to);
                    Ok(true)
                }
                (
                    ValidEntry::Moved { .. },
                    Node::Value {
                        value: SyntaxValue::Item(_),
                    },
                ) => Err(Error::LambdaToMustBeFeatures),
                (ValidEntry::Moved { .. }, Node::Projection { .. }) => Err(Error::LambdaToMustBeFeatures),
                (
                    ValidEntry::Lambda {
                        from: new,
                        to: from,
                        project,
                    },
                    Node::Value { value: to },
                ) => {
                    let to = match to {
                        SyntaxValue::Features(to) => to,
                        _ => Err(Error::LambdaToMustBeFeatures)?,
                    };
                    if self.push_lambda_features(from, to)? && project {
                        let ignore = new.get_features_right();
                        self.push_projection(ignore.clone());
                    }

                    let new = self.valid_entry_to_node(*new)?;
                    self.push(new);
                    Ok(true)
                }
                (
                    ValidEntry::Lambda {
                        from: a,
                        to: b,
                        project: project_ab,
                    },
                    Node::Lambda { from: c, to: d },
                ) => {
                    let b = ValidEntry::from(b);
                    if self.push_lexicon_lambda(b, *d)? && project_ab {
                        let ignore = (*a).get_features_right();
                        self.push_projection(ignore.clone());
                    }

                    let a = self.valid_entry_to_node(*a)?;
                    self.push_lambda(c, a);
                    Ok(true)
                }
                (_, Node::Projection { .. }) => Err(Error::LambdaToMustBeFeatures),
            }
        }
    }

    impl<K: Clone + Ord> CognitiveModel<K> for LambdaModel<K> {
        fn init(target: FeatureSet<K>) -> Self {
            let mut model = Self::new();
            let target = Node::from(target);
            model.push(target);
            model
        }

        fn understood(&self) -> bool {
            self.is_empty()
        }

        fn demand(&self) -> bool {
            match self.peek() {
                Some(node) => matches!(node, Node::Value { .. } | Node::Lambda { .. }),
                None => false,
            }
        }

        fn receive(&mut self, token: K) -> super::super::super::error::Result<()> {
            self.begin_step();
            self.log_event(DerivationOp::MergeToken {
                token: token.clone(),
            });
            let from = SyntaxValue::from(token);
            let to = self.pop_node()?;
            self.push_lambda(from, to);
            Ok(())
        }

        fn wonder(&self) -> Option<&SyntaxValue<K>> {
            match self.peek() {
                Some(Node::Lambda { from, .. }) => Some(from),
                _ => None,
            }
        }

        fn decide(&mut self, entry: LexiconEntry<K>) -> super::super::super::error::Result<()> {
            self.begin_step();
            self.log_event(DerivationOp::MergeEntry {
                entry: entry.clone(),
            });
            let target = self.pop_node()?;
            match target {
                Node::Lambda {
                    from: original_from,
                    to,
                } => match (original_from, entry) {
                    (
                        SyntaxValue::Features(from_fs),
                        LexiconEntry::Functional {
                            to: from,
                            project: entry_project,
                        },
                    ) => {
                        let mut from = ValidEntry::try_from(from)?;
                        if let Some(ignore_fs) = entry_project {
                            let onto_fs = from.get_features_right_mut();
                            FeatureSet::project(&from_fs, onto_fs, &ignore_fs)
                                .map_err(Error::Syntax)?;
                        }
                        self.push_lexicon_lambda(from, *to)?;
                    }
                    (SyntaxValue::Item(_), LexiconEntry::Lexical(from)) => {
                        let from = ValidEntry::try_from(from)?;
                        self.push_lexicon_lambda(from, *to)?;
                    }
                    _ => Err(Error::QueryAndEntryTypeMismatch)?,
                },
                _ => Err(Error::ApplyEntryToNonLambda)?,
            }
            Ok(())
        }
    }
}

impl<K: Display + Clone + Ord> Display for LambdaModel<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = self
            .expects
            .iter()
            .rev()
            .map(|expect| format!("{}", expect))
            .collect::<Vec<String>>()
            .join(", ");
        let mut pending = self
            .pending_chain_requirements()
            .iter()
            .map(|features| format!("{}", *features))
            .collect::<Vec<_>>();
        pending.sort();

        if pending.is_empty() {
            write!(f, "{}", formatted)
        } else {
            write!(f, "{} | pending: {}", formatted, pending.join(" || "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_entry_introduces_pending_chain() {
        let mut model: LambdaModel<&str> = LambdaModel::default();

        let mut moved = FeatureSet::new();
        moved.insert("aux", None);
        let mut target = FeatureSet::new();
        target.insert("tp", None);

        let inserted = model
            .push_lexicon_lambda(
                ValidEntry::Moved {
                    from: moved.clone(),
                },
                Node::from(target),
            )
            .unwrap();

        assert!(inserted);
        assert_eq!(model.pending_chain_count(), 1);
    }

    #[test]
    fn compatible_features_discharge_one_pending_chain() {
        let mut model: LambdaModel<&str> = LambdaModel::default();

        let mut moved = FeatureSet::new();
        moved.insert("aux", None);
        let mut target = FeatureSet::new();
        target.insert("tp", None);
        model
            .push_lexicon_lambda(
                ValidEntry::Moved {
                    from: moved.clone(),
                },
                Node::from(target),
            )
            .unwrap();
        assert_eq!(model.pending_chain_count(), 1);

        let mut provided = FeatureSet::new();
        provided.insert("aux", None);
        provided.insert("tense", Some("past"));
        let mut onto = FeatureSet::new();
        onto.insert("aux", None);
        let _ = model.push_lambda_features(provided, onto).unwrap();

        assert_eq!(model.pending_chain_count(), 0);
    }

    #[test]
    fn incompatible_features_do_not_discharge_chain() {
        let mut model: LambdaModel<&str> = LambdaModel::default();

        let mut moved = FeatureSet::new();
        moved.insert("aux", None);
        let mut target = FeatureSet::new();
        target.insert("tp", None);
        model
            .push_lexicon_lambda(
                ValidEntry::Moved {
                    from: moved.clone(),
                },
                Node::from(target),
            )
            .unwrap();
        assert_eq!(model.pending_chain_count(), 1);

        let mut provided = FeatureSet::new();
        provided.insert("case", Some("obj"));
        let mut onto = FeatureSet::new();
        onto.insert("case", Some("obj"));
        let _ = model.push_lambda_features(provided, onto).unwrap();

        assert_eq!(model.pending_chain_count(), 1);
    }

    #[test]
    fn chain_discharge_is_one_time() {
        let mut model: LambdaModel<&str> = LambdaModel::default();
        model.enable_trace();

        let mut moved = FeatureSet::new();
        moved.insert("aux", None);
        let mut target = FeatureSet::new();
        target.insert("tp", None);
        model
            .push_lexicon_lambda(
                ValidEntry::Moved {
                    from: moved.clone(),
                },
                Node::from(target),
            )
            .unwrap();
        assert_eq!(model.pending_chain_count(), 1);

        let mut provided = FeatureSet::new();
        provided.insert("aux", None);
        let mut onto = FeatureSet::new();
        onto.insert("aux", None);
        let _ = model.push_lambda_features(provided.clone(), onto.clone()).unwrap();
        let _ = model.push_lambda_features(provided, onto).unwrap();

        let chains = model.chain_lifecycle_records();
        let discharged = chains
            .iter()
            .filter(|chain| chain.status == "discharged")
            .count();
        assert_eq!(discharged, 1);
    }
}
