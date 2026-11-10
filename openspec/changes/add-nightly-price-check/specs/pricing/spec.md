# Spec Delta

## MODIFIED Requirements

### Requirement: Built-in prices
The binary SHALL contain the repository's `prices.toml` as its built-in prices. They SHALL cover the Anthropic models and the OpenAI models that Codex uses and that have a public price. The source SHALL be written next to them, with the date they were last brought in line with it. The nightly check (see price-updates) keeps them in line.

#### Scenario: No configuration at all
- **WHEN** neither a user price file nor `--prices` exists
- **THEN** every request to a built-in model is priced from the built-in prices
