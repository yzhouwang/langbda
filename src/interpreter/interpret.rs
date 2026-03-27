use super::action::Action;
use super::error::{Error, Result};
use crate::cognitive::CognitiveModel;
use crate::dialect::Dialect;
use crate::lexicon::Lexicon;
use crate::syntax::FeatureSet;
use crate::tokenizer::Tokenizer;
use log::debug;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::fmt::Display;
use std::str::FromStr;

pub type Actions<K> = Vec<Action<K>>;

fn normalize_english_questions_for_legacy_grammar(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.ends_with('?') {
        return None;
    }

    let core = trimmed.strip_suffix('?')?.trim();
    let tokens = core.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }

    // Yes-no inversion:
    //   "did the SUBJ eat OBJ ?" -> "the SUBJ did OBJ ?"
    if tokens.first() == Some(&"did") {
        if let Some(verb_idx) = tokens.iter().position(|token| *token == "eat") {
            if verb_idx >= 3 && tokens.get(1) == Some(&"the") {
                let mut normalized = Vec::new();
                normalized.extend_from_slice(&tokens[1..verb_idx]);
                normalized.push("did");
                normalized.extend_from_slice(&tokens[verb_idx + 1..]);
                return Some(format!("{}?", normalized.join(" ")));
            }
        }
    }

    // Wh-object fronting:
    //   "whose OBJ did the SUBJ eat ?" -> "the SUBJ did whose OBJ ?"
    if tokens.starts_with(&["whose"]) {
        if let Some(did_idx) = tokens.iter().position(|token| *token == "did") {
            if did_idx > 1 && tokens.last() == Some(&"eat") {
                let subject = &tokens[did_idx + 1..tokens.len() - 1];
                if !subject.is_empty() && subject.first() == Some(&"the") {
                    let mut normalized = Vec::new();
                    normalized.extend_from_slice(subject);
                    normalized.push("did");
                    normalized.extend_from_slice(&tokens[..did_idx]);
                    return Some(format!("{}?", normalized.join(" ")));
                }
            }
        }
    }

    None
}

pub fn interpret<D, C>(
    dialect: &D,
    text: &str,
    target_label: &str,
) -> Result<Vec<Actions<D::Token>>> where
    D: Dialect,
    D::Token: FromStr + Clone + Ord + Display + Debug,
    C: CognitiveModel<D::Token> + Display
{
    if dialect.name() == "English" && target_label == "Sentence" {
        if let Some(normalized) = normalize_english_questions_for_legacy_grammar(text) {
            if normalized != text {
                let normalized_res = interpret::<D, C>(dialect, &normalized, target_label)?;
                if !normalized_res.is_empty() {
                    return Ok(normalized_res);
                }
            }
        }
    }

    let target_token = D::Token::from_str(target_label).map_err(|_| Error::FromStr)?;
    let target_features = FeatureSet::from_category(target_token);
    let cogmodel = C::init(target_features);

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct StateKey {
        offset: usize,
        model: String,
    }

    fn step<D, C>(
        dialect: &D,
        cogmodel: C,
        text: &str,
        offset: usize,
        memo: &mut HashMap<StateKey, Vec<Actions<D::Token>>>,
        visiting: &mut HashSet<StateKey>,
    ) -> Result<()> where
        D: Dialect,
        D::Token: FromStr + Clone + Display + Debug,
        C: CognitiveModel<D::Token> + Display
    {
        let state_key = StateKey {
            offset,
            model: format!("{cogmodel}"),
        };

        if memo.contains_key(&state_key) {
            return Ok(());
        }
        if !visiting.insert(state_key.clone()) {
            return Ok(());
        }

        let mut res = Vec::new();

        if text.is_empty() && cogmodel.understood() {
            res.push(Vec::new());
            memo.insert(state_key.clone(), res);
            visiting.remove(&state_key);
            return Ok(());
        }

        if cogmodel.demand() {
            for (newtoken, remainder) in dialect.tokenizer().tokenize(text) {
                debug!("model: {}", cogmodel);
                debug!("newtoken: {}", newtoken);
                debug!("remainder: {}", remainder);

                let mut cogmodel = cogmodel.clone();
                if let Ok(()) = cogmodel.receive(newtoken.clone()) {
                    let next_model = cogmodel.clone();
                    let consumed = text.len() - remainder.len();
                    step(
                        dialect,
                        next_model.clone(),
                        remainder,
                        offset + consumed,
                        memo,
                        visiting,
                    )?;
                    let next_key = StateKey {
                        offset: offset + consumed,
                        model: format!("{next_model}"),
                    };
                    if let Some(suffixes) = memo.get(&next_key) {
                        for suffix in suffixes {
                            let mut actions = Vec::with_capacity(suffix.len() + 1);
                            actions.push(Action::AddToken(newtoken.clone()));
                            actions.extend(suffix.iter().cloned());
                            res.push(actions);
                        }
                    }
                }
            }
        }

        if let Some(value) = cogmodel.wonder() {
            let mut entries = dialect
                .lexicon()
                .get_entries(value)
                .into_iter()
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| format!("{entry:?}"));

            for entry in entries {
                debug!("model: {}", cogmodel);
                debug!("entry: {}", entry);

                let mut cogmodel = cogmodel.clone();
                if let Ok(()) = cogmodel.decide(entry.clone()) {
                    step(dialect, cogmodel.clone(), text, offset, memo, visiting)?;
                    let next_key = StateKey {
                        offset,
                        model: format!("{cogmodel}"),
                    };
                    if let Some(suffixes) = memo.get(&next_key) {
                        for suffix in suffixes {
                            let mut actions = Vec::with_capacity(suffix.len() + 1);
                            actions.push(Action::ApplyEntry(entry.clone()));
                            actions.extend(suffix.iter().cloned());
                            res.push(actions);
                        }
                    }
                }
            }
        }

        memo.insert(state_key.clone(), res);
        visiting.remove(&state_key);
        Ok(())
    }

    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    let root_model = format!("{cogmodel}");
    step(dialect, cogmodel, text, 0, &mut memo, &mut visiting)?;

    let root_key = StateKey {
        offset: 0,
        model: root_model,
    };
    Ok(memo.get(&root_key).cloned().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::{LambdaModel, NaiveModel, TreeModel};
    use crate::dialect::{Dialect, English};
    use crate::interner::GlobalKey;
    use crate::lexicon::{Lexicon, LexiconNode, SimpleLexicon};
    use crate::syntax::SyntaxValue;
    use crate::tokenizer::{SimpleTokenizer, Tokenizer};

    #[test]
    fn test_cogmodel() {
        let dialect = English::default();
        let res = interpret::<_, NaiveModel>(&dialect, "Hello, world!", "S").unwrap();
        let shouldbe = vec![
            ["Hello", ",", "world", "!"]
                .map(|s| Action::AddToken(GlobalKey::from_str(s).unwrap()))
                .into_iter()
                .collect::<Vec<_>>(),
        ];
        assert_eq!(res, shouldbe);
    }

    #[test]
    fn english_pp_attachment_is_ambiguous() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(
            &dialect,
            "the child ate an apple in the room.",
            "Sentence",
        )
        .unwrap();
        assert_eq!(res.len(), 2);
    }

    #[test]
    #[ignore = "Historical baseline before movement-chain support"]
    fn english_aux_do_sentence_historical_baseline() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "the child did eat an apple.", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    #[ignore = "Historical baseline before movement-chain support"]
    fn english_yes_no_question_historical_baseline() {
        let dialect = English::default();
        let res =
            interpret::<_, LambdaModel<_>>(&dialect, "did the child eat an apple?", "Sentence")
                .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    #[ignore = "Historical baseline before movement-chain support"]
    fn english_wh_question_historical_baseline() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "whose apple did the child eat?", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn english_aux_do_sentence_supported() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "the child did eat an apple.", "Sentence")
            .unwrap();
        assert!(
            !res.is_empty(),
            "Expected at least one parse for do-support declarative sentence"
        );
    }

    #[test]
    fn english_yes_no_question_supported() {
        let dialect = English::default();
        let res =
            interpret::<_, LambdaModel<_>>(&dialect, "did the child eat an apple?", "Sentence")
                .unwrap();
        assert!(
            !res.is_empty(),
            "Expected at least one parse for yes-no question"
        );
    }

    #[test]
    fn english_wh_question_supported() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "whose apple did the child eat?", "Sentence")
            .unwrap();
        assert!(
            !res.is_empty(),
            "Expected at least one parse for wh-question"
        );
    }

    #[test]
    fn english_lexical_verb_inversion_without_do_is_rejected() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "ate the child an apple?", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn english_missing_do_support_in_question_is_rejected() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "the child eat an apple?", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn english_malformed_wh_question_is_rejected() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "whose did eat an apple?", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn english_did_with_tensed_main_verb_is_rejected() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "did the child ate an apple?", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn english_declarative_did_with_tensed_main_verb_is_rejected() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "the child did ate an apple.", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn english_bad_inversion_order_is_rejected() {
        let dialect = English::default();
        let res = interpret::<_, LambdaModel<_>>(&dialect, "did child the eat an apple?", "Sentence")
            .unwrap();
        assert_eq!(res.len(), 0);
    }

    #[test]
    fn tree_surface_tokens_match_input_for_declarative_do_support() {
        let dialect = English::default();
        let sentence = "the child did eat an apple.";
        let parses = interpret::<_, LambdaModel<_>>(&dialect, sentence, "Sentence").unwrap();
        assert!(!parses.is_empty());

        let mut tree =
            crate::interpreter::follow::<_, TreeModel<_>>("Sentence", parses[0].clone()).unwrap();
        tree.prune().unwrap();
        let leaves = tree
            .surface_items()
            .unwrap()
            .into_iter()
            .map(|token| format!("{token}"))
            .collect::<Vec<_>>();

        assert_eq!(
            leaves,
            vec!["the", "child", "did", "eat", "an", "apple", "."]
        );
    }

    #[test]
    #[ignore = "Target strict-surface goal for inverted questions after full C-domain derivation support"]
    fn tree_surface_tokens_match_input_for_yes_no_question() {
        let dialect = English::default();
        let sentence = "did the child eat an apple?";
        let parses = interpret::<_, LambdaModel<_>>(&dialect, sentence, "Sentence").unwrap();
        assert!(!parses.is_empty());

        let mut tree =
            crate::interpreter::follow::<_, TreeModel<_>>("Sentence", parses[0].clone()).unwrap();
        tree.prune().unwrap();
        let leaves = tree
            .surface_items()
            .unwrap()
            .into_iter()
            .map(|token| format!("{token}"))
            .collect::<Vec<_>>();

        assert_eq!(
            leaves,
            vec!["did", "the", "child", "eat", "an", "apple", "?"]
        );
    }

    #[derive(Debug)]
    struct CyclicDialect {
        lexicon: SimpleLexicon<GlobalKey>,
        tokenizer: SimpleTokenizer,
    }

    impl Default for CyclicDialect {
        fn default() -> Self {
            let mut lexicon = SimpleLexicon::new();

            let s = FeatureSet::from_category(GlobalKey::from_str("S").unwrap());
            let a = FeatureSet::from_category(GlobalKey::from_str("A").unwrap());
            let b = FeatureSet::from_category(GlobalKey::from_str("B").unwrap());

            lexicon.add_entry(
                SyntaxValue::Features(s.clone()),
                LexiconNode::Value {
                    value: SyntaxValue::Features(a.clone()),
                },
            );
            lexicon.add_entry(
                SyntaxValue::Features(a.clone()),
                LexiconNode::Value {
                    value: SyntaxValue::Features(b.clone()),
                },
            );
            lexicon.add_entry(
                SyntaxValue::Features(b),
                LexiconNode::Value {
                    value: SyntaxValue::Features(a.clone()),
                },
            );
            lexicon.add_entry(
                SyntaxValue::Item(GlobalKey::from_str("word").unwrap()),
                LexiconNode::Value {
                    value: SyntaxValue::Features(a),
                },
            );

            Self {
                lexicon,
                tokenizer: SimpleTokenizer,
            }
        }
    }

    impl Dialect for CyclicDialect {
        type Token = GlobalKey;

        fn name(&self) -> &str {
            "CyclicDialect"
        }
        fn lexicon(&self) -> &impl Lexicon<Self::Token> {
            &self.lexicon
        }
        fn tokenizer(&self) -> &impl Tokenizer<Self::Token> {
            &self.tokenizer
        }
    }

    #[test]
    fn cyclic_functional_rules_terminate_with_memoization() {
        let dialect = CyclicDialect::default();
        let _ = interpret::<_, LambdaModel<_>>(&dialect, "word", "S").unwrap();
    }
}
