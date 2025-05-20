use chumsky::{prelude::*, Parser};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum VariableSubstitutionError {
    #[error("unable to substitute variables {0:#?}")]
    SubstitutionError(Vec<String>),
    #[error("variable expansion recursion limit (100) reached")]
    RecursionLimit,
}

pub(crate) trait HasVariables {
    fn get_variable(&self, input: &str) -> Option<String>;
}

/// Helper for variable substitution with environment variables
pub(crate) struct Environment {}

impl HasVariables for Environment {
    fn get_variable(&self, input: &str) -> Option<String> {
        std::env::var(input).ok()
    }
}

fn parser<'a>(
    variables: &'a [&'a dyn HasVariables],
) -> impl Parser<'a, &'a str, String, chumsky::extra::Err<Rich<'a, char>>> {
    recursive(|p| {
        just('$')
            .ignore_then(p.delimited_by(just('('), just(')')))
            .try_map(|s: String, span| {
                variables
                    .iter()
                    .find_map(|v| v.get_variable(&s))
                    .ok_or(Rich::custom(
                        span,
                        format!("could not expand variable $({})", s),
                    ))
            })
            .or(none_of("$)").repeated().at_least(1).collect::<String>())
            .repeated()
            .collect::<Vec<_>>()
            .map(|v| v.concat())
    })
}

/// Substitute variables of the format `$(VAR)`, where `VAR` is the variable name
/// passed to `get_var`.
pub(crate) fn substitute(
    variables: &[&dyn HasVariables],
    input: &str,
) -> Result<String, VariableSubstitutionError> {
    let p = |input: &str| {
        parser(variables).parse(input).into_result().map_err(|err| {
            VariableSubstitutionError::SubstitutionError(
                err.into_iter().map(|e| e.to_string()).collect(),
            )
        })
    };

    let mut output = p(input)?;
    let mut next = p(&output)?;

    let mut count = 0;

    while next != output {
        if count > 100 {
            return Err(VariableSubstitutionError::RecursionLimit);
        }

        count += 1;
        output = next;
        next = p(&output)?;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestVariables;

    impl HasVariables for TestVariables {
        fn get_variable(&self, input: &str) -> Option<String> {
            match input {
                "TEST_VAR" => Some("foo".into()),
                "RECURSIVE_VAR" => Some("$(TEST_VAR)".into()),
                "FLATTEN_VAR" => Some("TEST_VAR".into()),
                "INFINITELY_RECURSIVE_1" => Some("$(INFINITELY_RECURSIVE_2)".into()),
                "INFINITELY_RECURSIVE_2" => Some("$(INFINITELY_RECURSIVE_1)".into()),
                "EMPTY_STRING" => Some("".into()),
                _ => None,
            }
        }
    }

    #[test]
    fn substitute_helper() {
        assert_eq!(
            substitute(&[&TestVariables], "$(TEST_VAR)").unwrap(),
            "foo".to_string()
        );
        substitute(&[&TestVariables], "$(UNRECOGNISED)").unwrap_err();
    }

    #[test]
    fn flattened_variables() {
        let input = "$($(FLATTEN_VAR))";
        let expected = "foo";
        let result = substitute(&[&TestVariables], input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn recursive_variables() {
        let input = "$(RECURSIVE_VAR)";
        let expected = "foo";
        let result = substitute(&[&TestVariables], input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn infinitely_recursive_variables() {
        let input = "$(INFINITELY_RECURSIVE_1)";
        let result = substitute(&[&TestVariables], input);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VariableSubstitutionError::RecursionLimit
        ));
    }

    #[test]
    fn complex_substitution() {
        let input = " '$(TEST_VAR)' = $($(FLATTEN_VAR)) $(RECURSIVE_VAR);";
        let expected = " 'foo' = foo foo;";
        let result = substitute(&[&TestVariables], input).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn substitute_with_empty_string() {
        assert_eq!(
            substitute(&[&TestVariables], "$(EMPTY_STRING)").unwrap(),
            "".to_string()
        );
    }

    #[test]
    fn substitute_empty_string() {
        assert_eq!(substitute(&[&TestVariables], "").unwrap(), "".to_string());
    }
}
