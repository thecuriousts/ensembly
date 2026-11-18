//! Query set with golden answers by construction (honest low scores on nl/para/fts-zero).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyedQuery {
    pub query: String,
    pub expect_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshPairQuery {
    pub query: String,
    pub expect_first_key: String,
    pub old_key: String,
    pub new_key: String,
    /// `Z` | `+00:00` | `+02:00` | `+05:30` for old/new last_seen strings.
    pub tz_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixQuery {
    pub prefix: String,
    pub expect_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySet {
    pub unique: Vec<KeyedQuery>,
    pub natural_phrase: Vec<KeyedQuery>,
    pub paraphrase_extra: Vec<KeyedQuery>,
    pub fts_zero_hit: Vec<KeyedQuery>,
    pub fresh_pairs: Vec<FreshPairQuery>,
    pub prefixes: Vec<PrefixQuery>,
}

const TZ_STYLES: [&str; 4] = ["Z", "+00:00", "+02:00", "+05:30"];

impl QuerySet {
    pub fn build() -> Self {
        let unique = (0..50)
            .map(|i| KeyedQuery {
                query: format!("uqterm-bench-{i:04}"),
                expect_key: format!("bench/unique/{i:04}"),
            })
            .collect();

        let natural_phrase = (0..20)
            .map(|i| {
                let anchor = format!("anchor-nl-{i:02}");
                KeyedQuery {
                    query: format!("who what when did the {anchor} happen on the desk at laptop-1"),
                    expect_key: format!("bench/nl/{i:02}"),
                }
            })
            .collect();

        let paraphrase_extra = (0..10)
            .map(|i| {
                let topic = format!("para-topic-{i:02}");
                KeyedQuery {
                    query: format!("{topic} certref unlisted-noise-term-{i:02}"),
                    expect_key: format!("bench/para/{i:02}"),
                }
            })
            .collect();

        let fts_zero_hit = (0..10)
            .map(|i| KeyedQuery {
                query: format!("ftszero topic-{i:02} absent-token-{i:02}"),
                expect_key: format!("bench/fts0/{i:02}"),
            })
            .collect();

        let fresh_pairs = (0..20)
            .map(|i| {
                let token = format!("sharedpair-{i:04}");
                FreshPairQuery {
                    query: token.clone(),
                    expect_first_key: format!("bench/pair/{i:04}/current"),
                    old_key: format!("bench/pair/{i:04}/stale"),
                    new_key: format!("bench/pair/{i:04}/current"),
                    tz_style: TZ_STYLES[i % TZ_STYLES.len()].to_string(),
                }
            })
            .collect();

        let prefixes = (0..20)
            .map(|i| {
                let prefix = format!("bench/pfx/{i:02}/");
                PrefixQuery {
                    prefix: prefix.clone(),
                    expect_keys: vec![format!("bench/pfx/{i:02}/leaf")],
                }
            })
            .collect();

        Self {
            unique,
            natural_phrase,
            paraphrase_extra,
            fts_zero_hit,
            fresh_pairs,
            prefixes,
        }
    }

    pub fn planted_row_count() -> usize {
        50 + 20 + 10 + 10 + 40 + 20
    }
}
