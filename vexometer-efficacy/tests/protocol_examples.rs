// SPDX-License-Identifier: MPL-2.0
//! The efficacy protocol's own example documents are the fixtures.
//!
//! These tests read `vexometer/docs/EFFICACY-PROTOCOL.adoc`, extract its
//! `vexometer-efficacy-v2` and `vexometer-frontier-v1` example JSON blocks,
//! and require that (a) the validator accepts both, and (b) the evaluator
//! reproduces the efficacy example byte-for-value from its raw inputs.
//! If the protocol's examples and this implementation ever drift apart,
//! these tests fail loudly.

use std::collections::BTreeMap;
use std::path::PathBuf;

use vexometer_efficacy::*;

fn protocol_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vexometer/docs/EFFICACY-PROTOCOL.adoc")
}

/// Extract every `----`-delimited block that parses as JSON, keyed by its
/// `version` field.
fn protocol_examples() -> BTreeMap<String, serde_json::Value> {
    let text = std::fs::read_to_string(protocol_path())
        .expect("EFFICACY-PROTOCOL.adoc must be readable from the monorepo layout");
    let mut examples = BTreeMap::new();
    let mut block: Option<String> = None;
    for line in text.lines() {
        if line.trim_end() == "----" {
            match block.take() {
                None => block = Some(String::new()),
                Some(content) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(version) = v.get("version").and_then(|s| s.as_str()) {
                            examples.insert(version.to_string(), v);
                        }
                    }
                }
            }
        } else if let Some(content) = block.as_mut() {
            content.push_str(line);
            content.push('\n');
        }
    }
    examples
}

fn efficacy_example() -> serde_json::Value {
    protocol_examples()
        .remove(EFFICACY_VERSION)
        .expect("protocol must contain a vexometer-efficacy-v2 example")
}

fn frontier_example() -> serde_json::Value {
    protocol_examples()
        .remove(FRONTIER_VERSION)
        .expect("protocol must contain a vexometer-frontier-v1 example")
}

// ---------------------------------------------------------------------------
// The protocol's examples must validate
// ---------------------------------------------------------------------------

#[test]
fn protocol_efficacy_example_is_valid() {
    let problems = validate_efficacy(&efficacy_example());
    assert!(
        problems.is_empty(),
        "the protocol's own efficacy-v2 example failed validation:\n{}",
        problems.join("\n")
    );
}

#[test]
fn protocol_frontier_example_is_valid() {
    let problems = validate_frontier(&frontier_example());
    assert!(
        problems.is_empty(),
        "the protocol's own frontier-v1 example failed validation:\n{}",
        problems.join("\n")
    );
}

// ---------------------------------------------------------------------------
// The evaluator must reproduce the efficacy example from raw inputs
// ---------------------------------------------------------------------------

fn measurement(metrics: &[(&str, serde_json::Value)], passed: u32, total: u32) -> Measurement {
    let doc = serde_json::json!({
        "scenario_set": "sha256:6b2f...",
        "metrics": metrics.iter().cloned().collect::<BTreeMap<_, _>>(),
        "probes": { "total": total, "passed": passed },
    });
    serde_json::from_value(doc).expect("measurement fixture must parse")
}

fn example_baseline() -> Measurement {
    measurement(
        &[
            ("LPS", serde_json::json!(0.41)),
            ("TII", serde_json::json!(0.33)),
            ("EFR", serde_json::json!(0.19)),
            ("PQ", serde_json::json!(0.28)),
            ("TAI", serde_json::json!(0.15)),
            ("ICS", serde_json::json!(0.22)),
            ("CII", serde_json::json!(0.31)),
            ("SRS", serde_json::json!(0.26)),
            ("SFR", serde_json::json!(0.24)),
            ("RCI", serde_json::json!(0.30)),
        ],
        12,
        13,
    )
}

fn example_after() -> Measurement {
    measurement(
        &[
            (
                "LPS",
                serde_json::json!({"score": 0.17, "std_dev": 0.09, "confidence": 0.95, "p_value": 0.001}),
            ),
            (
                "TII",
                serde_json::json!({"score": 0.22, "std_dev": 0.07, "confidence": 0.95, "p_value": 0.004}),
            ),
            ("EFR", serde_json::json!(0.20)),
            ("PQ", serde_json::json!(0.26)),
            ("TAI", serde_json::json!(0.15)),
            ("ICS", serde_json::json!(0.23)),
            ("CII", serde_json::json!(0.35)),
            ("SRS", serde_json::json!(0.26)),
            ("SFR", serde_json::json!(0.25)),
            ("RCI", serde_json::json!(0.30)),
        ],
        12,
        13,
    )
}

#[test]
fn evaluator_reproduces_protocol_efficacy_example() {
    let targets = vec!["LPS".to_string(), "TII".to_string()];
    let eval = evaluate(&example_baseline(), &example_after(), &targets)
        .expect("the protocol example inputs must evaluate cleanly");

    assert_eq!(eval.verdict, Verdict::AcceptWithWarning);
    assert_eq!(eval.warned_metrics, vec!["CII".to_string()]);

    let meta = ReportMeta {
        satellite: "vex-verbosity-compressor".into(),
        evaluation_date: "2026-09-01".into(),
        sample_size: 500,
        scenario_set: "sha256:6b2f...".into(),
        methodology: "A/B testing with vexometer validation".into(),
        traces_available: true,
        verdict_notes: Some(
            "CII regressed by 0.04 -- compression removes content in long-form code \
             scenarios. Must be declared in satellite README."
                .into(),
        ),
        frontier_record: Some("frontier/LPS-2026-09-01.json".into()),
    };
    let (report, warnings) = build_report(&eval, &meta).expect("report must build");

    // Two targets with a singular frontier_record is exactly the D1d
    // ambiguity; the tool must surface it as a warning, not guess.
    assert!(
        warnings.iter().any(|w| w.contains("D1d")),
        "expected a D1d plurality warning, got: {warnings:?}"
    );

    let produced = serde_json::to_value(&report).expect("report must serialise");
    assert_eq!(
        produced,
        efficacy_example(),
        "the emitted report must equal the protocol's example value-for-value"
    );

    // And what the tool emits must itself validate.
    let problems = validate_efficacy(&produced);
    assert!(problems.is_empty(), "emitted report invalid: {problems:?}");
}

// ---------------------------------------------------------------------------
// Open D1 questions must be refusals, not guesses
// ---------------------------------------------------------------------------

fn expect_ruling(result: Result<Evaluation, EfficacyError>, question: &str) {
    match result {
        Err(EfficacyError::AwaitingRuling { question: q, .. }) => assert_eq!(q, question),
        other => panic!("expected AwaitingRuling({question}), got {other:?}"),
    }
}

#[test]
fn zero_baseline_target_awaits_d1a() {
    let mut baseline = example_baseline();
    baseline.metrics.insert(
        "LPS".into(),
        serde_json::from_value(serde_json::json!(0.0)).unwrap(),
    );
    expect_ruling(
        evaluate(&baseline, &example_after(), &["LPS".to_string()]),
        "D1a",
    );
}

#[test]
fn probe_gate_disagreement_awaits_d1b() {
    let mut ids: Vec<String> = (1..=13).map(|i| format!("P{i:02}")).collect();
    ids.sort();
    let before: BTreeMap<String, bool> = ids.iter().map(|id| (id.clone(), id != "P13")).collect();
    // Two baseline-passing probes regress, one baseline-failing probe now
    // passes: aggregate rate drops by exactly one probe (gate passes) while
    // the identity gate counts two regressions (gate fails).
    let after_r: BTreeMap<String, bool> = ids
        .iter()
        .map(|id| {
            let v = match id.as_str() {
                "P01" | "P02" => false,
                "P13" => true,
                _ => before[id],
            };
            (id.clone(), v)
        })
        .collect();

    let mut baseline = example_baseline();
    baseline.probes.results = Some(before);
    let mut after = example_after();
    after.probes.passed = 11;
    after.probes.results = Some(after_r);

    expect_ruling(
        evaluate(&baseline, &after, &["LPS".to_string(), "TII".to_string()]),
        "D1b",
    );
}

#[test]
fn mixed_target_improvement_awaits_d1c() {
    let mut after = example_after();
    // TII regresses while LPS improves.
    after.metrics.insert(
        "TII".into(),
        serde_json::from_value(serde_json::json!(0.34)).unwrap(),
    );
    expect_ruling(
        evaluate(
            &example_baseline(),
            &after,
            &["LPS".to_string(), "TII".to_string()],
        ),
        "D1c",
    );
}

// ---------------------------------------------------------------------------
// Verdict precedence
// ---------------------------------------------------------------------------

#[test]
fn reject_null_outranks_all_other_rejects() {
    let baseline = example_baseline();
    let mut after = example_after();
    // Target worse, capability destroyed, collateral blown, D_ISA positive.
    after.metrics.insert(
        "LPS".into(),
        serde_json::from_value(serde_json::json!(0.60)).unwrap(),
    );
    after.metrics.insert(
        "CII".into(),
        serde_json::from_value(serde_json::json!(0.90)).unwrap(),
    );
    after.probes.passed = 5;
    let eval = evaluate(&baseline, &after, &["LPS".to_string()]).unwrap();
    assert_eq!(eval.verdict, Verdict::RejectNull);
}

#[test]
fn reject_capability_outranks_collateral_and_net() {
    let baseline = example_baseline();
    let mut after = example_after();
    after.metrics.insert(
        "CII".into(),
        serde_json::from_value(serde_json::json!(0.90)).unwrap(),
    );
    after.probes.passed = 5;
    let eval = evaluate(&baseline, &after, &["LPS".to_string(), "TII".to_string()]).unwrap();
    assert_eq!(eval.verdict, Verdict::RejectCapability);
}

#[test]
fn clean_improvement_accepts() {
    let baseline = example_baseline();
    let mut after = example_after();
    // Remove the CII warning-band regression.
    after.metrics.insert(
        "CII".into(),
        serde_json::from_value(serde_json::json!(0.31)).unwrap(),
    );
    let eval = evaluate(&baseline, &after, &["LPS".to_string(), "TII".to_string()]).unwrap();
    assert_eq!(eval.verdict, Verdict::Accept);
    assert!(eval.warned_metrics.is_empty());
}

// ---------------------------------------------------------------------------
// Frontier invariants
// ---------------------------------------------------------------------------

fn frontier_eval(after_lps: f64, capability_pass: u32) -> Evaluation {
    let baseline = example_baseline();
    let mut after = example_after();
    after.metrics.insert(
        "LPS".into(),
        serde_json::from_value(serde_json::json!(after_lps)).unwrap(),
    );
    // Keep CII clean so verdicts differ only via capability.
    after.metrics.insert(
        "CII".into(),
        serde_json::from_value(serde_json::json!(0.31)).unwrap(),
    );
    after.probes.passed = capability_pass;
    evaluate(&baseline, &after, &["LPS".to_string(), "TII".to_string()]).unwrap()
}

#[test]
fn rejected_attempt_with_higher_gap_does_not_advance_frontier() {
    let mut record = FrontierRecord::new(
        "LPS",
        "claude-opus-5",
        "2026-09-01T10:30:00Z",
        "sha256:6b2f...",
        0.41,
        46.2,
        12.0 / 13.0,
    );

    // Attempt 1: modest accepted improvement.
    let a1 = frontier_eval(0.34, 12);
    assert!(a1.verdict.advances_frontier());
    record
        .append(
            "vex-verbosity-compressor",
            "strip_filler=true",
            &a1,
            "sha256:6b2f...",
        )
        .unwrap();
    let f1 = record.attempts[0].frontier;
    assert!(f1 > 0.0);

    // Attempt 2: much larger gap, but capability-rejected.
    let a2 = frontier_eval(0.05, 10);
    assert_eq!(a2.verdict, Verdict::RejectCapability);
    record
        .append(
            "vex-verbosity-compressor",
            "aggressive=true",
            &a2,
            "sha256:6b2f...",
        )
        .unwrap();
    assert_eq!(
        record.attempts[1].frontier, f1,
        "a rejected attempt must not advance the frontier"
    );

    // Attempt 3: accepted and better than attempt 1.
    let a3 = frontier_eval(0.17, 12);
    assert!(a3.verdict.advances_frontier());
    record
        .append(
            "vex-verbosity-compressor",
            "preserve_code=true",
            &a3,
            "sha256:6b2f...",
        )
        .unwrap();
    assert!(record.attempts[2].frontier > f1);

    assert_eq!(record.methods_tried, 3);
    assert_eq!(record.methods_rejected, 1);
    assert_eq!(record.frontier_final, record.attempts[2].frontier);

    // The record the writer produces must satisfy the validator.
    let doc = serde_json::to_value(&record).unwrap();
    let problems = validate_frontier(&doc);
    assert!(problems.is_empty(), "written record invalid: {problems:?}");
}

#[test]
fn scenario_set_mismatch_is_rejected() {
    let mut record = FrontierRecord::new(
        "LPS",
        "claude-opus-5",
        "2026-09-01T10:30:00Z",
        "sha256:aaaa",
        0.41,
        46.2,
        12.0 / 13.0,
    );
    let a1 = frontier_eval(0.34, 12);
    let err = record
        .append("vex-verbosity-compressor", "cfg", &a1, "sha256:bbbb")
        .unwrap_err();
    assert!(err.to_string().contains("identical"));
}

// ---------------------------------------------------------------------------
// The validator must reject corrupted documents
// ---------------------------------------------------------------------------

#[test]
fn validator_catches_frontier_advanced_by_reject() {
    let mut doc = frontier_example();
    // Corrupt attempt 2 (reject_capability) to advance the frontier.
    doc["attempts"][1]["frontier"] = serde_json::json!(0.780);
    let problems = validate_frontier(&doc);
    assert!(
        problems.iter().any(|p| p.contains("invariant")),
        "expected an invariant violation, got: {problems:?}"
    );
}

#[test]
fn validator_catches_wrong_verdict() {
    let mut doc = efficacy_example();
    doc["verdict"] = serde_json::json!("accept");
    let problems = validate_efficacy(&doc);
    assert!(
        problems.iter().any(|p| p.contains("acceptance rule")),
        "expected a verdict mismatch, got: {problems:?}"
    );
}

#[test]
fn validator_catches_bad_gap_arithmetic() {
    let mut doc = efficacy_example();
    doc["target_metrics"]["LPS"]["gap_closed"] = serde_json::json!(0.9);
    let problems = validate_efficacy(&doc);
    assert!(
        problems.iter().any(|p| p.contains("gap_closed")),
        "expected a gap_closed mismatch, got: {problems:?}"
    );
}

#[test]
fn validator_catches_missing_collateral_coverage() {
    let mut doc = efficacy_example();
    doc["collateral_metrics"]
        .as_object_mut()
        .unwrap()
        .remove("RCI");
    let problems = validate_efficacy(&doc);
    assert!(
        problems.iter().any(|p| p.contains("RCI")),
        "expected missing-coverage problem, got: {problems:?}"
    );
}
