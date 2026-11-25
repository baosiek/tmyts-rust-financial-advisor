use std::collections::BTreeMap;

use advisor_llm::pricing::{LongContextPrice, ModelPrice, PriceTable};
use advisor_llm::types::Usage;

fn flat() -> ModelPrice {
    ModelPrice {
        input_per_mtok: 2.0,
        output_per_mtok: 10.0,
        cache_write_per_mtok: 2.5,
        cache_read_per_mtok: 0.1,
        long_context: None,
    }
}

fn usage(input: u32, output: u32) -> Usage {
    Usage {
        input_tokens: input,
        output_tokens: output,
        ..Usage::default()
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn flat_price_per_million_tokens() {
    // 1M input at $2 + 100k output at $10 = $2 + $1.
    assert!(close(flat().cost_usd(&usage(1_000_000, 100_000)), 3.0));
}

#[test]
fn cache_tokens_have_their_own_prices() {
    let usage = Usage {
        input_tokens: 0,
        output_tokens: 0,
        cache_creation_input_tokens: 1_000_000,
        cache_read_input_tokens: 1_000_000,
    };
    assert!(close(flat().cost_usd(&usage), 2.5 + 0.1));
}

#[test]
fn long_context_price_applies_to_the_whole_call_above_the_threshold() {
    let price = ModelPrice {
        long_context: Some(LongContextPrice {
            above_input_tokens: 100_000,
            input_per_mtok: 4.0,
            output_per_mtok: 20.0,
            cache_write_per_mtok: 5.0,
            cache_read_per_mtok: 0.2,
        }),
        ..flat()
    };
    assert!(
        close(price.cost_usd(&usage(100_000, 0)), 0.2),
        "at the threshold"
    );
    assert!(close(price.cost_usd(&usage(200_000, 0)), 0.8), "above it");
}

#[test]
fn table_matches_exact_id_then_longest_prefix() {
    let mut models = BTreeMap::new();
    models.insert("claude-x".to_string(), flat());
    models.insert(
        "claude-x-5".to_string(),
        ModelPrice {
            input_per_mtok: 1.0,
            ..flat()
        },
    );
    let table = PriceTable::new(models);

    assert_eq!(table.price("claude-x").unwrap().input_per_mtok, 2.0);
    assert_eq!(
        table.price("claude-x-5-20261001").unwrap().input_per_mtok,
        1.0
    );
    assert!(table.price("other-model").is_none());
    assert_eq!(table.cost_usd("other-model", &usage(10, 10)), None);
}
