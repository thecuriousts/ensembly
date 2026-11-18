//! Query set with golden answers (50 unique, 20 fresh pairs, 20 prefix).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniqueQuery {
    pub query: String,
    pub expect_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshPairQuery {
    pub query: String,
    pub expect_first_key: String,
    pub old_key: String,
    pub new_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixQuery {
    pub prefix: String,
    pub expect_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySet {
    pub unique: Vec<UniqueQuery>,
    pub fresh_pairs: Vec<FreshPairQuery>,
    pub prefixes: Vec<PrefixQuery>,
}

impl QuerySet {
    pub fn build() -> Self {
        let unique = (0..50)
            .map(|i| UniqueQuery {
                query: format!("uqterm-bench-{i:04}"),
                expect_key: format!("bench/unique/{i:04}"),
            })
            .collect();
        let fresh_pairs = (0..20)
            .map(|i| FreshPairQuery {
                query: format!("sharedpair-{i:04}"),
                expect_first_key: format!("bench/pair/{i:04}/new"),
                old_key: format!("bench/pair/{i:04}/old"),
                new_key: format!("bench/pair/{i:04}/new"),
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
            fresh_pairs,
            prefixes,
        }
    }
}
