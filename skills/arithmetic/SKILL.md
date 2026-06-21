---
name: arithmetic
description: Guidance for evaluating arithmetic expressions. Activate when the user asks to compute, calculate, or evaluate a math expression with numbers and the operators + - * / and parentheses. Enables the calculator tool.
allowed-tools: calculator
---

# Arithmetic

When the user asks to evaluate an arithmetic expression, use the `calculator` tool with the expression exactly as given.

## When to use

- The user wants a numeric result for an expression like `2 + 3 * 4` or `(10 / 4) + 1`.
- The expression uses numbers and the operators `+`, `-`, `*`, `/`, and parentheses.

## How to use the calculator tool

- Pass the expression **verbatim** so operator precedence and parentheses are preserved — do not pre-compute, simplify, or drop parentheses.
- Present the returned result to the user clearly.

## Examples

| Request | Pass to `calculator` | Result |
|---|---|---|
| "What is 2 + 3 * 4?" | `2 + 3 * 4` | `14` |
| "Compute (1 + 2) * (3 + 4)" | `(1 + 2) * (3 + 4)` | `21` |
| "10 divided by 4" | `10 / 4` | `2.5` |

## When NOT to use

- Non-arithmetic requests — don't force a calculation where none was asked for.
