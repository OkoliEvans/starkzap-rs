use std::{env, fs, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TokenEntry {
    function: String,
    symbol: String,
    name: String,
    decimals: u8,
    address: String,
}

#[derive(Debug, Deserialize)]
struct TokenPresetFile {
    mainnet: Vec<TokenEntry>,
    sepolia: Vec<TokenEntry>,
}

#[derive(Debug, Deserialize)]
struct ValidatorEntry {
    name: String,
    staker_address: String,
}

#[derive(Debug, Deserialize)]
struct ValidatorPresetFile {
    mainnet: Vec<ValidatorEntry>,
    sepolia: Vec<ValidatorEntry>,
}

fn main() {
    let tokens_path = PathBuf::from("codegen/presets/tokens.json");
    let validators_path = PathBuf::from("codegen/presets/validators.json");

    println!("cargo:rerun-if-changed={}", tokens_path.display());
    println!("cargo:rerun-if-changed={}", validators_path.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));

    let tokens: TokenPresetFile = serde_json::from_str(
        &fs::read_to_string(&tokens_path).expect("failed to read token preset source"),
    )
    .expect("failed to parse token preset source");
    let validators: ValidatorPresetFile = serde_json::from_str(
        &fs::read_to_string(&validators_path).expect("failed to read validator preset source"),
    )
    .expect("failed to parse validator preset source");

    fs::write(
        out_dir.join("tokens_generated.rs"),
        render_tokens(&tokens),
    )
    .expect("failed to write generated tokens");
    fs::write(
        out_dir.join("validators_generated.rs"),
        render_validators(&validators),
    )
    .expect("failed to write generated validators");
}

fn render_tokens(file: &TokenPresetFile) -> String {
    format!(
        "{}\n{}",
        render_token_module("mainnet", &file.mainnet),
        render_token_module("sepolia", &file.sepolia),
    )
}

fn render_token_module(name: &str, entries: &[TokenEntry]) -> String {
    let mut out = String::new();
    out.push_str(&format!("pub mod {name} {{\n"));
    out.push_str("    use super::*;\n\n");

    for entry in entries {
        assert_identifier(&entry.function);
        out.push_str(&format!(
            "    pub fn {}() -> Token {{\n        Token::new(\n            \"{}\",\n            \"{}\",\n            {},\n            Felt::from_hex_unchecked(\"{}\"),\n        )\n    }}\n\n",
            entry.function, entry.symbol, entry.name, entry.decimals, entry.address
        ));
    }

    let all_items = entries
        .iter()
        .map(|entry| format!("{}()", entry.function))
        .collect::<Vec<_>>()
        .join(", ");

    out.push_str(&format!(
        "    pub fn all() -> Vec<Token> {{\n        vec![{all_items}]\n    }}\n\n"
    ));
    out.push_str(
        "    pub fn by_symbol(symbol: &str) -> Option<Token> {\n        all().into_iter().find(|t| t.symbol.eq_ignore_ascii_case(symbol))\n    }\n",
    );
    out.push_str("}\n");
    out
}

fn render_validators(file: &ValidatorPresetFile) -> String {
    format!(
        "{}\n{}",
        render_validator_fn("mainnet_validators", &file.mainnet),
        render_validator_fn("sepolia_validators", &file.sepolia),
    )
}

fn render_validator_fn(name: &str, entries: &[ValidatorEntry]) -> String {
    let body = entries
        .iter()
        .map(|entry| {
            format!(
                "        Validator::new(\n            \"{}\",\n            Felt::from_hex_unchecked(\"{}\"),\n        )",
                entry.name, entry.staker_address
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");

    format!("pub fn {name}() -> Vec<Validator> {{\n    vec![\n{body}\n    ]\n}}\n")
}

fn assert_identifier(value: &str) {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        panic!("empty identifier in preset data");
    };

    if !(first == '_' || first.is_ascii_alphabetic()) {
        panic!("invalid identifier in preset data: {value}");
    }

    if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        panic!("invalid identifier in preset data: {value}");
    }
}
