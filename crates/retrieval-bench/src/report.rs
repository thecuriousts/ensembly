//! JSON and markdown report output.

use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::harness::SizeReport;
use crate::score::{QualityScores, TypeRecall};

#[derive(Debug, Serialize)]
pub struct BenchReport {
    pub format: &'static str,
    pub generated_at: String,
    pub git_sha: Option<String>,
    pub sizes: Vec<SizeReport>,
}

pub fn write_outputs(
    out_dir: &Path,
    reports: &[SizeReport],
    git_sha: Option<String>,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let doc = BenchReport {
        format: "retrieval-bench-v1",
        generated_at: Utc::now().to_rfc3339(),
        git_sha,
        sizes: reports.to_vec(),
    };
    let json = serde_json::to_string_pretty(&doc)?;
    std::fs::write(out_dir.join("results.json"), json)?;
    std::fs::write(out_dir.join("summary.md"), markdown_summary(&doc))?;
    Ok(())
}

fn markdown_summary(doc: &BenchReport) -> String {
    let mut md = String::from("# Retrieval bench summary\n\n");
    md.push_str(&format!("Generated: {}\n\n", doc.generated_at));
    if let Some(sha) = &doc.git_sha {
        md.push_str(&format!("Commit: `{sha}`\n\n"));
    }

    md.push_str("## Quality (recall by query type)\n\n");
    md.push_str("| size | type | recall@5 | recall@10 | fresh-first |\n");
    md.push_str("|---:|---|---:|---:|---:|\n");
    for s in &doc.sizes {
        let q = &s.quality;
        for (name, tr) in type_rows(q) {
            md.push_str(&format!(
                "| {} | {} | {:.3} | {:.3} | — |\n",
                s.size, name, tr.recall_at_5, tr.recall_at_10
            ));
        }
        md.push_str(&format!(
            "| {} | fresh_first | — | — | {:.3} |\n",
            s.size, q.fresh_first_accuracy
        ));
    }

    md.push_str("\n## Latency (ms)\n\n");
    md.push_str("| size | op | p50 | p95 |\n");
    md.push_str("|---:|---|---:|---:|\n");
    for s in &doc.sizes {
        for (name, lat) in [
            ("prefix_read", &s.latency_ms.prefix_read),
            ("fts_search", &s.latency_ms.fts_search),
            ("fallback_text_search", &s.latency_ms.fallback_text_search),
            ("pulse_list", &s.latency_ms.pulse_list),
            ("dangling_check", &s.latency_ms.dangling_supersession_check),
            ("record_snapshot", &s.latency_ms.record_log_snapshot),
            ("single_commit", &s.latency_ms.single_event_commit),
            ("feed_last_10", &s.latency_ms.feed_last_10),
        ] {
            md.push_str(&format!(
                "| {} | {} | {:.3} | {:.3} |\n",
                s.size, name, lat.p50, lat.p95
            ));
        }
    }

    md.push_str("\n## Pulse index (measured on this machine)\n\n");
    for s in &doc.sizes {
        let p = &s.pulse_index;
        md.push_str(&format!("### size {} — `{}`\n\n", s.size, p.machine));
        md.push_str("| query | indexes | p50 ms | p95 ms |\n");
        md.push_str("|---|---|---:|---:|\n");
        let rows = [
            ("pulse_list", "no", &p.pulse_list_no_index),
            ("pulse_list", "yes", &p.pulse_list_with_index),
            ("dangling NOT EXISTS", "no", &p.dangling_legacy_no_index),
            ("dangling JOIN", "no", &p.dangling_new_no_index),
            ("dangling NOT EXISTS", "yes", &p.dangling_legacy_with_index),
            ("dangling JOIN", "yes", &p.dangling_new_with_index),
        ];
        for (query, indexes, lat) in rows {
            md.push_str(&format!(
                "| {query} | {indexes} | {:.3} | {:.3} |\n",
                lat.p50, lat.p95
            ));
        }
        md.push('\n');
    }

    md
}

fn type_rows(q: &QualityScores) -> [(&str, &TypeRecall); 5] {
    [
        ("unique", &q.unique),
        ("natural_phrase", &q.natural_phrase),
        ("paraphrase_extra", &q.paraphrase_extra),
        ("fts_zero_hit", &q.fts_zero_hit),
        ("prefix", &q.prefix),
    ]
}
