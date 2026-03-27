use crate::lexicon::{
    LexiconNode,
    LexiconNode::{Lambda, Moved, Value},
};
use crate::syntax::{FeatureSet, SyntaxValue::Features};

pub enum ValidEntry<K> {
    Features(FeatureSet<K>),
    Moved {
        from: FeatureSet<K>,
    },
    Lambda {
        from: Box<ValidEntry<K>>,
        to: FeatureSet<K>,
        project: bool,
    },
}

impl<K> ValidEntry<K> {
    pub fn get_features_right(&self) -> &FeatureSet<K> {
        match self {
            ValidEntry::Features(fs) => fs,
            ValidEntry::Moved { from } => from,
            ValidEntry::Lambda { to, .. } => to,
        }
    }
    pub fn get_features_right_mut(&mut self) -> &mut FeatureSet<K> {
        match self {
            ValidEntry::Features(fs) => fs,
            ValidEntry::Moved { from } => from,
            ValidEntry::Lambda { to, .. } => to,
        }
    }
}

impl<K> From<FeatureSet<K>> for ValidEntry<K> {
    fn from(fs: FeatureSet<K>) -> Self {
        ValidEntry::Features(fs)
    }
}

use crate::syntax::SyntaxValue;
impl<K> TryInto<SyntaxValue<K>> for ValidEntry<K> {
    type Error = super::Error;
    fn try_into(self) -> Result<SyntaxValue<K>, Self::Error> {
        match self {
            ValidEntry::Features(fs) => Ok(Features(fs)),
            ValidEntry::Moved { .. } => Err(Self::Error::TypeConversion),
            ValidEntry::Lambda { .. } => Err(Self::Error::TypeConversion),
        }
    }
}

impl<K> TryFrom<LexiconNode<K>> for ValidEntry<K> {
    type Error = super::Error;

    fn try_from(node: LexiconNode<K>) -> Result<Self, Self::Error> {
        let err = Self::Error::TypeConversion;
        match node {
            Value {
                value: Features(fs),
            } => Ok(ValidEntry::Features(fs)),
            Moved { from } => Ok(ValidEntry::Moved { from }),
            Lambda { from, to, project } => {
                let from_entry = Self::try_from(*from)?;
                let to = match *to {
                    Value {
                        value: Features(fs),
                    } => fs,
                    Moved { from: moved_fs } => moved_fs,
                    _ => Err(err)?,
                };
                Ok(ValidEntry::Lambda {
                    from: Box::new(from_entry),
                    to,
                    project,
                })
            }
            _ => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moved_is_preserved_as_moved_entry() {
        let mut fs = FeatureSet::new();
        fs.insert("xp", None);
        fs.insert("case", Some("obj"));

        let moved = LexiconNode::Moved { from: fs.clone() };
        let valid = ValidEntry::try_from(moved).unwrap();
        match valid {
            ValidEntry::Moved { from: parsed } => assert_eq!(parsed, fs),
            ValidEntry::Features(_) => panic!("MOVED should not flatten to plain features"),
            ValidEntry::Lambda { .. } => panic!("MOVED should remain moved"),
        }
    }

    #[test]
    #[ignore = "Historical baseline before movement-chain semantics"]
    fn moved_historical_flattening_baseline() {
        let mut fs = FeatureSet::new();
        fs.insert("xp", None);
        fs.insert("case", Some("obj"));

        let moved = LexiconNode::Moved { from: fs.clone() };
        let valid = ValidEntry::try_from(moved).unwrap();
        match valid {
            ValidEntry::Features(parsed) => assert_eq!(parsed, fs),
            _ => panic!("Historical behavior used to flatten MOVED into Features"),
        }
    }

    #[test]
    fn lambda_with_moved_target_is_accepted_as_feature_target() {
        let mut left = FeatureSet::new();
        left.insert("v", None);

        let mut right = FeatureSet::new();
        right.insert("dp", Some("obj"));

        let node = LexiconNode::Lambda {
            from: Box::new(LexiconNode::Value {
                value: Features(left.clone()),
            }),
            to: Box::new(LexiconNode::Moved {
                from: right.clone(),
            }),
            project: false,
        };

        let valid = ValidEntry::try_from(node).unwrap();
        match valid {
            ValidEntry::Lambda { from, to, project } => {
                assert!(!project);
                assert_eq!(to, right);
                match *from {
                    ValidEntry::Features(fs) => assert_eq!(fs, left),
                    ValidEntry::Moved { .. } => panic!("left side should not become MOVED"),
                    ValidEntry::Lambda { .. } => panic!("left side should stay feature-based"),
                }
            }
            ValidEntry::Features(_) => panic!("expected lambda entry"),
            ValidEntry::Moved { .. } => panic!("expected lambda entry"),
        }
    }
}
