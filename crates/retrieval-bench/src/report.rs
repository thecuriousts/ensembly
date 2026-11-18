//! JSON and markdown report output.

use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::harness::SizeReport;

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
    md.push_str("| size | op | p50 ms | p95 ms | recall@5 | recall@10 | fresh-first |\n");
    md.push_str("|---:|---|---:|---:|---:|---:|---:|\n");
    for s in &doc.sizes {
        let q = &s.quality;
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
                "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
                s.size,
                name,
                lat.p50,
                lat.p95,
                q.recall_at_5,
                q.recall_at_10,
                q.fresh_first_accuracy
            ));
        }
    }
    md
}
