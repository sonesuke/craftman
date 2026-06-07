use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;

use crate::core::skill::SkillExecutor;

/// A calculator skill that evaluates basic arithmetic expressions.
///
/// Supports `+`, `-`, `*`, `/` and parentheses.
pub struct Calculator;

#[async_trait]
impl SkillExecutor for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<String> {
        let expression = arguments["expression"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'expression' parameter"))?;

        let result = eval_expression(expression)?;
        Ok(format!("{result}"))
    }
}

/// Evaluate a simple arithmetic expression.
///
/// Supports `+`, `-`, `*`, `/`, parentheses, and integer/decimal numbers.
/// Returns an error for any character that is not part of the expression grammar.
fn eval_expression(expr: &str) -> Result<f64> {
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_additive(&tokens, &mut pos)?;
    if pos < tokens.len() {
        anyhow::bail!("Unexpected token after expression: {:?}", tokens[pos]);
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(expr: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut num_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        num_str.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let n: f64 = num_str
                    .parse()
                    .with_context(|| format!("Invalid number: {num_str}"))?;
                tokens.push(Token::Number(n));
            }
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '/' => {
                chars.next();
                tokens.push(Token::Slash);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            _ => anyhow::bail!("Unexpected character in expression: '{ch}'"),
        }
    }

    Ok(tokens)
}

/// Parse additive: expr (('+' | '-') expr)*
fn parse_additive(tokens: &[Token], pos: &mut usize) -> Result<f64> {
    let mut result = parse_multiplicative(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                result += parse_multiplicative(tokens, pos)?;
            }
            Token::Minus => {
                *pos += 1;
                result -= parse_multiplicative(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(result)
}

/// Parse multiplicative: expr (('*' | '/') expr)*
fn parse_multiplicative(tokens: &[Token], pos: &mut usize) -> Result<f64> {
    let mut result = parse_primary(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Star => {
                *pos += 1;
                result *= parse_primary(tokens, pos)?;
            }
            Token::Slash => {
                *pos += 1;
                let divisor = parse_primary(tokens, pos)?;
                if divisor == 0.0 {
                    anyhow::bail!("Division by zero");
                }
                result /= divisor;
            }
            _ => break,
        }
    }
    Ok(result)
}

/// Parse primary: number | '(' expr ')' | unary minus
fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<f64> {
    if *pos >= tokens.len() {
        anyhow::bail!("Unexpected end of expression");
    }

    match &tokens[*pos] {
        Token::Number(n) => {
            let val = *n;
            *pos += 1;
            Ok(val)
        }
        Token::LParen => {
            *pos += 1;
            let result = parse_additive(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                anyhow::bail!("Missing closing parenthesis");
            }
            *pos += 1;
            Ok(result)
        }
        Token::Minus => {
            *pos += 1;
            Ok(-parse_primary(tokens, pos)?)
        }
        _ => anyhow::bail!("Unexpected token: {:?}", tokens[*pos]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_calculator_execute() {
        let calc = Calculator;
        let args = serde_json::json!({"expression": "2 + 3 * 4"});
        let result = calc.execute(args).await.unwrap();
        assert_eq!(result, "14");
    }

    #[tokio::test]
    async fn test_calculator_with_parens() {
        let calc = Calculator;
        let args = serde_json::json!({"expression": "2 * (3 + 4)"});
        let result = calc.execute(args).await.unwrap();
        assert_eq!(result, "14");
    }

    #[tokio::test]
    async fn test_calculator_division() {
        let calc = Calculator;
        let args = serde_json::json!({"expression": "100 / 4 - 5"});
        let result = calc.execute(args).await.unwrap();
        assert_eq!(result, "20");
    }

    #[tokio::test]
    async fn test_calculator_negative() {
        let calc = Calculator;
        let args = serde_json::json!({"expression": "-5 + 10"});
        let result = calc.execute(args).await.unwrap();
        assert_eq!(result, "5");
    }

    #[tokio::test]
    async fn test_calculator_missing_param() {
        let calc = Calculator;
        let args = serde_json::json!({});
        assert!(calc.execute(args).await.is_err());
    }
}
