use proptest::prelude::*;
use rogg_core::filter::{split_expression, join_expression, Term};

proptest! {
    #[test]
    fn editor_cycle_is_idempotent(input in "[a-b |\\\\]{0,40}") {
        // The editor cycle split → join → split should be idempotent for
        // all expressions that can be produced by split_expression.
        let original_split = split_expression(&input);
        let terms: Vec<Term> = original_split.iter()
            .map(|s| Term::include(s.clone()))
            .collect();
        let joined = join_expression(&terms);
        let re_split = split_expression(&joined);

        prop_assert_eq!(re_split, original_split,
            "Editor cycle split→join→split should be idempotent\ninput: {:?}",
            input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_limitation_programmatic_backslash_pipe_pipe() {
        // A term whose text contains the literal sequence \|\| (only possible when
        // constructed programmatically) re-parses as ||. This is the known limitation:
        // split(join([Term::include(r"a\|\|b")])) produces ["a||b"] not ["a\|\|b"].
        assert_eq!(
            split_expression(&join_expression(&[Term::include(r"a\|\|b")])),
            vec!["a||b".to_string()]
        );
    }
}
