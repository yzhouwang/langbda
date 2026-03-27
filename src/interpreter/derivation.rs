use super::action::Action;
use super::error::{Error, Result};
use crate::cognitive::{ChainLifecycleRecord, CognitiveModel, DerivationStepRecord, LambdaModel};
use crate::syntax::FeatureSet;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivationArtifact {
    pub sentence: String,
    pub target: String,
    pub parse_index: usize,
    pub tokens: Vec<String>,
    pub derivation_steps: Vec<DerivationStepRecord>,
    pub chain_events: Vec<ChainLifecycleRecord>,
    pub well_formed: bool,
    pub unresolved_chains: Vec<String>,
}

pub fn build_derivation_artifact<K>(
    sentence: &str,
    target: &str,
    parse_index: usize,
    actions: &[Action<K>],
) -> Result<DerivationArtifact>
where
    K: Clone + FromStr + Ord + Debug + Display,
{
    let target_token = K::from_str(target).map_err(|_| Error::FromStr)?;
    let target_features = FeatureSet::from_category(target_token);
    let mut model = LambdaModel::init(target_features);
    model.enable_trace();

    let mut tokens = Vec::new();
    for action in actions {
        match action {
            Action::AddToken(token) => {
                tokens.push(format!("{token}"));
                model.receive(token.clone())?;
            }
            Action::ApplyEntry(entry) => {
                model.decide(entry.clone())?;
            }
        }
    }

    let unresolved_chains = model.unresolved_chain_requirements();
    Ok(DerivationArtifact {
        sentence: sentence.to_string(),
        target: target.to_string(),
        parse_index,
        tokens,
        derivation_steps: model.derivation_step_records(),
        chain_events: model.chain_lifecycle_records(),
        well_formed: model.understood() && unresolved_chains.is_empty(),
        unresolved_chains,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::English;
    use crate::interpreter::interpret;

    #[test]
    fn derivation_json_round_trip() {
        let dialect = English::default();
        let sentence = "the child ate an apple.";
        let target = "Sentence";
        let parses = interpret::<_, LambdaModel<_>>(&dialect, sentence, target).unwrap();
        assert!(!parses.is_empty());

        let artifact = build_derivation_artifact(sentence, target, 1, &parses[0]).unwrap();
        let serialized = serde_json::to_string(&artifact).unwrap();
        let restored: DerivationArtifact = serde_json::from_str(&serialized).unwrap();
        assert_eq!(artifact, restored);
        assert_eq!(artifact.well_formed, artifact.unresolved_chains.is_empty());
        assert!(!artifact.tokens.is_empty());
        assert!(!artifact.derivation_steps.is_empty());
    }
}
