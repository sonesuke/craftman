//! Calculator tool — evaluates an arithmetic expression safely.
//!
//! Hand-rolled recursive-descent parser supporting `+ - * /`, parentheses,
//! unary `+`/`-`, and integer/decimal numbers. No external dependency and no
//! `eval`-style arbitrary evaluation: the accepted grammar is fixed and tiny,
//! so there is no code-execution attack surface.

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;

use super::Tool;

pub struct Calculator;

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Evaluate an arithmetic expression and return the numeric result. \
         Supports +, -, *, / and parentheses, e.g. \"2 + 3 * 4\"."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The arithmetic expression to evaluate, e.g. \"2 + 3 * 4\""
                }
            },
            "required": ["expression"]
        })
    }

    async fn call(&self, args: serde_json::Value) -> Result<String> {
        let expr = args
            .get("expression")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("missing string argument 'expression'"))?;
        let value = eval(expr)?;
        Ok(format_result(value))
    }
}

/// Render a result without a trailing `.0` for whole numbers.
fn format_result(value: f64) -> String {
    let s = value.to_string();
    if let Some(stripped) = s.strip_suffix(".0") {
        stripped.to_string()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Recursive-descent parser
//
//   expr   = term   (('+' | '-') term)*
//   term   = factor (('*' | '/') factor)*
//   factor = number | '(' expr ')' | ('+' | '-') factor
// ---------------------------------------------------------------------------

fn eval(input: &str) -> Result<f64> {
    let mut p = Parser {
        chars: input.chars().collect(),
        pos: 0,
    };
    p.skip_ws();
    let result = p.parse_expr()?;
    p.skip_ws();
    if p.pos != p.chars.len() {
        bail!("unexpected character at position {}", p.pos);
    }
    Ok(result)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn parse_expr(&mut self) -> Result<f64> {
        let mut left = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => {
                    self.pos += 1;
                    left += self.parse_term()?;
                }
                Some('-') => {
                    self.pos += 1;
                    left -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<f64> {
        let mut left = self.parse_factor()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    left *= self.parse_factor()?;
                }
                Some('/') => {
                    self.pos += 1;
                    let r = self.parse_factor()?;
                    if r == 0.0 {
                        bail!("division by zero");
                    }
                    left /= r;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<f64> {
        self.skip_ws();
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let v = self.parse_expr()?;
                self.skip_ws();
                match self.peek() {
                    Some(')') => {
                        self.pos += 1;
                        Ok(v)
                    }
                    _ => bail!("expected ')'"),
                }
            }
            Some('-') => {
                self.pos += 1;
                Ok(-self.parse_factor()?)
            }
            Some('+') => {
                self.pos += 1;
                self.parse_factor()
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.parse_number(),
            Some(c) => bail!("unexpected character '{c}'"),
            None => bail!("unexpected end of expression"),
        }
    }

    fn parse_number(&mut self) -> Result<f64> {
        let start = self.pos;
        let mut saw_dot = false;
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c.is_ascii_digit() {
                self.pos += 1;
            } else if c == '.' && !saw_dot {
                saw_dot = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse::<f64>()
            .map_err(|_| anyhow!("invalid number '{s}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_call_end_to_end() {
        let calc = Calculator;
        let out = calc
            .call(serde_json::json!({"expression": "2 + 3 * 4"}))
            .await
            .unwrap();
        assert_eq!(out, "14");
    }

    #[tokio::test]
    async fn test_call_missing_argument() {
        let calc = Calculator;
        assert!(calc.call(serde_json::json!({})).await.is_err());
    }

    #[test]
    fn test_eval_precedence_and_parens() {
        assert_eq!(eval("2 + 3 * 4").unwrap(), 14.0);
        assert_eq!(eval("2 * (3 + 4)").unwrap(), 14.0);
        assert_eq!(eval("(1 + 2) * (3 + 4)").unwrap(), 21.0);
        assert_eq!(eval("10 / 4").unwrap(), 2.5);
        assert_eq!(eval("-5 + 3").unwrap(), -2.0);
        assert_eq!(eval("100").unwrap(), 100.0);
    }

    #[test]
    fn test_eval_errors() {
        assert!(eval("2 / 0").is_err(), "division by zero");
        assert!(eval("1 & 2").is_err(), "unexpected character");
        assert!(eval("2 * (3 + 4").is_err(), "unbalanced parens");
        assert!(eval("").is_err(), "empty expression");
    }

    #[test]
    fn test_format_result() {
        assert_eq!(format_result(14.0), "14");
        assert_eq!(format_result(2.5), "2.5");
        assert_eq!(format_result(-2.0), "-2");
    }
}
