// SPDX-License-Identifier: MPL-2.0
//! Efficacy evaluator for the vexometer ISA efficacy protocol.
//!
//! Implements the computation and validation halves of
//! `vexometer/docs/EFFICACY-PROTOCOL.adoc`: `G_m`, collateral deltas,
//! `D_ISA`, the capability proxy, the six-verdict acceptance rule with its
//! precedence order, and `vexometer-frontier-v1` record maintenance.
//!
//! Where the protocol is normatively undecided (issue #69, questions
//! D1a-D1d), this crate refuses to guess: it returns an
//! `EfficacyError::AwaitingRuling` naming the open question instead of
//! silently picking a semantic.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The owner-ruling issue batching the six open normative questions.
pub const ISSUE_D1: &str = "https://github.com/hyperpolymath/vexometer/issues/69";

/// The ten ISA metrics with their default category weights, in canonical
/// order. Source of truth: `vexometer/docs/METRICS.adoc`, "Default
/// category weights". `D_ISA` in the protocol's worked example (-2.71) is
/// reproducible only with these values.
pub const METRIC_WEIGHTS: [(&str, f64); 10] = [
    ("TII", 1.0),
    ("LPS", 1.2),
    ("EFR", 1.5),
    ("PQ", 1.1),
    ("TAI", 0.8),
    ("ICS", 1.3),
    ("CII", 1.4),
    ("SRS", 1.2),
    ("SFR", 1.3),
    ("RCI", 1.1),
];

/// Collateral delta at or below this is compatible with `accept`.
pub const COLLATERAL_ACCEPT: f64 = 0.02;
/// Collateral delta in `(COLLATERAL_ACCEPT, COLLATERAL_WARN]` yields
/// `accept_with_warning`; above it, `reject_collateral`.
pub const COLLATERAL_WARN: f64 = 0.05;

const EPS: f64 = 1e-9;

pub fn weight_of(metric: &str) -> Option<f64> {
    METRIC_WEIGHTS
        .iter()
        .find(|(m, _)| *m == metric)
        .map(|(_, w)| *w)
}

pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

pub fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum EfficacyError {
    /// The computation requires an answer to an open D1 question.
    AwaitingRuling {
        question: &'static str,
        detail: String,
    },
    /// The input data is malformed or incomplete.
    Data(String),
}

impl fmt::Display for EfficacyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EfficacyError::AwaitingRuling { question, detail } => {
                write!(f, "awaiting ruling {question} (see {ISSUE_D1}): {detail}")
            }
            EfficacyError::Data(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

impl std::error::Error for EfficacyError {}

// ---------------------------------------------------------------------------
// Measurement inputs (tool input format; documented in the crate README)
// ---------------------------------------------------------------------------

/// One measurement pass: all ten metric scores plus the probe result,
/// against one content-addressed scenario set.
#[derive(Debug, Clone, Deserialize)]
pub struct Measurement {
    #[serde(default)]
    pub scenario_set: Option<String>,
    pub metrics: BTreeMap<String, MetricReading>,
    pub probes: ProbeMeasurement,
}

/// A metric score, optionally with distribution statistics.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MetricReading {
    Score(f64),
    Detailed {
        score: f64,
        #[serde(default)]
        std_dev: Option<f64>,
        #[serde(default)]
        confidence: Option<f64>,
        #[serde(default)]
        p_value: Option<f64>,
    },
}

impl MetricReading {
    pub fn score(&self) -> f64 {
        match self {
            MetricReading::Score(s) => *s,
            MetricReading::Detailed { score, .. } => *score,
        }
    }
    pub fn stats(&self) -> (Option<f64>, Option<f64>, Option<f64>) {
        match self {
            MetricReading::Score(_) => (None, None, None),
            MetricReading::Detailed {
                std_dev,
                confidence,
                p_value,
                ..
            } => (*std_dev, *confidence, *p_value),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeMeasurement {
    pub total: u32,
    pub passed: u32,
    /// Optional per-probe outcomes, keyed by probe id. When present in
    /// both measurements, the identity gate is cross-checked against the
    /// aggregate gate (see D1b).
    #[serde(default)]
    pub results: Option<BTreeMap<String, bool>>,
}

impl ProbeMeasurement {
    pub fn pass_rate(&self) -> f64 {
        f64::from(self.passed) / f64::from(self.total)
    }

    fn check(&self) -> Result<(), EfficacyError> {
        if self.total == 0 {
            return Err(EfficacyError::Data("probes.total must be > 0".into()));
        }
        if self.passed > self.total {
            return Err(EfficacyError::Data(
                "probes.passed exceeds probes.total".into(),
            ));
        }
        if let Some(results) = &self.results {
            let n = results.len() as u32;
            let p = results.values().filter(|v| **v).count() as u32;
            if n != self.total || p != self.passed {
                return Err(EfficacyError::Data(format!(
                    "probes.results ({p}/{n}) disagrees with probes.passed/total ({}/{})",
                    self.passed, self.total
                )));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Verdicts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Accept,
    AcceptWithWarning,
    RejectCollateral,
    RejectCapability,
    RejectNet,
    RejectNull,
    /// Verification-status sentinel for lifted v1 reports, outside the
    /// six-verdict acceptance table. Never emitted by this tool (v1->v2
    /// lifting is unimplemented pending ruling D1e).
    Unverified,
}

impl Verdict {
    pub fn is_reject(self) -> bool {
        matches!(
            self,
            Verdict::RejectCollateral
                | Verdict::RejectCapability
                | Verdict::RejectNet
                | Verdict::RejectNull
        )
    }
    pub fn advances_frontier(self) -> bool {
        matches!(self, Verdict::Accept | Verdict::AcceptWithWarning)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Accept => "accept",
            Verdict::AcceptWithWarning => "accept_with_warning",
            Verdict::RejectCollateral => "reject_collateral",
            Verdict::RejectCapability => "reject_capability",
            Verdict::RejectNet => "reject_net",
            Verdict::RejectNull => "reject_null",
            Verdict::Unverified => "unverified",
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TargetOutcome {
    pub baseline: f64,
    pub after: f64,
    pub gap_closed: f64,
    pub mean_reduction: f64,
    pub std_dev: Option<f64>,
    pub confidence: Option<f64>,
    pub p_value: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CollateralOutcome {
    pub baseline: f64,
    pub after: f64,
    pub delta: f64,
}

#[derive(Debug, Clone)]
pub struct CapabilityOutcome {
    pub probes_total: u32,
    pub pass_rate_before: f64,
    pub pass_rate_after: f64,
    pub capability_ok: bool,
    pub probes_regressed: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Evaluation {
    pub targets: BTreeMap<String, TargetOutcome>,
    pub collateral: BTreeMap<String, CollateralOutcome>,
    pub capability: CapabilityOutcome,
    /// Unrounded `D_ISA`.
    pub isa_delta_raw: f64,
    pub verdict: Verdict,
    /// Collateral metrics whose delta falls in the warning band
    /// `(COLLATERAL_ACCEPT, COLLATERAL_WARN]`.
    pub warned_metrics: Vec<String>,
}

impl Evaluation {
    /// Worst collateral regression, if any collateral metric exists.
    pub fn collateral_max(&self) -> Option<(String, f64)> {
        self.collateral
            .iter()
            .max_by(|a, b| a.1.delta.total_cmp(&b.1.delta))
            .map(|(m, c)| (m.clone(), c.delta))
    }
}

/// Compute `G_m`, all collateral deltas, `D_ISA`, the capability proxy,
/// and the verdict, per the protocol's acceptance rule and precedence
/// order (`reject_null > reject_capability > reject_collateral > reject_net`).
pub fn evaluate(
    baseline: &Measurement,
    after: &Measurement,
    targets: &[String],
) -> Result<Evaluation, EfficacyError> {
    if targets.is_empty() {
        return Err(EfficacyError::Data(
            "at least one target metric required".into(),
        ));
    }
    baseline.probes.check()?;
    after.probes.check()?;
    if baseline.probes.total != after.probes.total {
        return Err(EfficacyError::Data(format!(
            "probe suite size changed between measurements ({} vs {})",
            baseline.probes.total, after.probes.total
        )));
    }
    if let (Some(b), Some(a)) = (&baseline.scenario_set, &after.scenario_set) {
        if b != a {
            return Err(EfficacyError::Data(format!(
                "scenario_set mismatch: baseline {b} vs after {a}"
            )));
        }
    }

    // Every one of the ten metrics must be present in both measurements:
    // the protocol's workflow says "All ten metrics plus probe pass-rate
    // -- not only the target metric."
    for (metric, _) in METRIC_WEIGHTS {
        if !baseline.metrics.contains_key(metric) {
            return Err(EfficacyError::Data(format!(
                "baseline is missing metric {metric}; all ten ISA metrics are required"
            )));
        }
        if !after.metrics.contains_key(metric) {
            return Err(EfficacyError::Data(format!(
                "after is missing metric {metric}; all ten ISA metrics are required"
            )));
        }
    }
    for t in targets {
        if weight_of(t).is_none() {
            return Err(EfficacyError::Data(format!("unknown target metric {t}")));
        }
    }

    // Targets: G_m. A zero baseline makes G_m undefined -- open question D1a.
    let mut target_out = BTreeMap::new();
    let mut improvements = Vec::new();
    for t in targets {
        let b = baseline.metrics[t].score();
        let a_reading = &after.metrics[t];
        let a = a_reading.score();
        if b == 0.0 {
            return Err(EfficacyError::AwaitingRuling {
                question: "D1a",
                detail: format!(
                    "target metric {t} has baseline 0; G_m = (B_m - A_m) / B_m is undefined"
                ),
            });
        }
        let (std_dev, confidence, p_value) = a_reading.stats();
        target_out.insert(
            t.clone(),
            TargetOutcome {
                baseline: b,
                after: a,
                gap_closed: (b - a) / b,
                mean_reduction: b - a,
                std_dev,
                confidence,
                p_value,
            },
        );
        improvements.push(a < b - EPS);
    }

    // Collateral: every metric outside the target set.
    let mut collateral = BTreeMap::new();
    for (metric, _) in METRIC_WEIGHTS {
        if targets.iter().any(|t| t == metric) {
            continue;
        }
        let b = baseline.metrics[metric].score();
        let a = after.metrics[metric].score();
        collateral.insert(
            metric.to_string(),
            CollateralOutcome {
                baseline: b,
                after: a,
                delta: a - b,
            },
        );
    }

    // D_ISA over all ten metrics, targets included.
    let mut num = 0.0;
    let mut den = 0.0;
    for (metric, w) in METRIC_WEIGHTS {
        let b = baseline.metrics[metric].score();
        let a = after.metrics[metric].score();
        num += w * (a - b);
        den += w;
    }
    let isa_delta_raw = num / den * 100.0;

    // Capability proxy. The normative table defines the aggregate gate;
    // the prose sentence about "two or more probes" implies an identity
    // gate. When per-probe data lets both be computed and they disagree,
    // that is open question D1b.
    let tolerance = 1.0 / f64::from(baseline.probes.total);
    let rate_before = baseline.probes.pass_rate();
    let rate_after = after.probes.pass_rate();
    let aggregate_ok = rate_after >= rate_before - tolerance - EPS;

    let probes_regressed = match (&baseline.probes.results, &after.probes.results) {
        (Some(before), Some(after_r)) => {
            if before.keys().ne(after_r.keys()) {
                return Err(EfficacyError::Data(
                    "probe ids differ between baseline and after measurements".into(),
                ));
            }
            let regressed: Vec<String> = before
                .iter()
                .filter(|(id, passed)| **passed && !after_r[*id])
                .map(|(id, _)| id.clone())
                .collect();
            let identity_ok = regressed.len() <= 1;
            if identity_ok != aggregate_ok {
                return Err(EfficacyError::AwaitingRuling {
                    question: "D1b",
                    detail: format!(
                        "aggregate pass-rate gate says capability_ok={aggregate_ok} but \
                         per-probe identity gate says capability_ok={identity_ok} \
                         ({} baseline-passing probes regressed: {})",
                        regressed.len(),
                        regressed.join(", ")
                    ),
                });
            }
            Some(regressed)
        }
        _ => None,
    };

    let capability = CapabilityOutcome {
        probes_total: baseline.probes.total,
        pass_rate_before: rate_before,
        pass_rate_after: rate_after,
        capability_ok: aggregate_ok,
        probes_regressed,
    };

    // Target improvement. All improved / none improved are decidable; a
    // mixed outcome needs the multi-target acceptance rule -- open
    // question D1c.
    let all_improved = improvements.iter().all(|i| *i);
    let none_improved = improvements.iter().all(|i| !*i);
    if !all_improved && !none_improved {
        let detail: Vec<String> = targets
            .iter()
            .zip(&improvements)
            .map(|(t, i)| format!("{t}: {}", if *i { "improved" } else { "not improved" }))
            .collect();
        return Err(EfficacyError::AwaitingRuling {
            question: "D1c",
            detail: format!(
                "targets disagree on improvement ({}); the multi-target acceptance rule is undecided",
                detail.join(", ")
            ),
        });
    }

    let max_collateral = collateral
        .values()
        .map(|c| c.delta)
        .fold(f64::NEG_INFINITY, f64::max);
    let warned_metrics: Vec<String> = collateral
        .iter()
        .filter(|(_, c)| c.delta > COLLATERAL_ACCEPT + EPS && c.delta <= COLLATERAL_WARN + EPS)
        .map(|(m, _)| m.clone())
        .collect();

    // Acceptance rule with the protocol's precedence order.
    let verdict = if none_improved {
        Verdict::RejectNull
    } else if !capability.capability_ok {
        Verdict::RejectCapability
    } else if !collateral.is_empty() && max_collateral > COLLATERAL_WARN + EPS {
        Verdict::RejectCollateral
    } else if round2(isa_delta_raw) >= 0.0 {
        // Gate on the rounded value: it is what the report stores, and the
        // validator's recomputation must reach the same verdict.
        Verdict::RejectNet
    } else if !warned_metrics.is_empty() {
        Verdict::AcceptWithWarning
    } else {
        Verdict::Accept
    };

    Ok(Evaluation {
        targets: target_out,
        collateral,
        capability,
        isa_delta_raw,
        verdict,
        warned_metrics,
    })
}

// ---------------------------------------------------------------------------
// vexometer-efficacy-v2 report
// ---------------------------------------------------------------------------

pub const EFFICACY_VERSION: &str = "vexometer-efficacy-v2";
pub const FRONTIER_VERSION: &str = "vexometer-frontier-v1";
pub const PROBE_PROXY_PATH: &str = "data/probes/behavioural_probes.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetMetricReport {
    pub baseline: f64,
    pub after: f64,
    pub gap_closed: f64,
    pub mean_reduction: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub std_dev: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralMetricReport {
    pub baseline: f64,
    pub after: f64,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub proxy: String,
    pub probes_total: u32,
    pub pass_rate_before: f64,
    pub pass_rate_after: f64,
    pub capability_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probes_regressed: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficacyReport {
    pub version: String,
    pub satellite: String,
    pub evaluation_date: String,
    pub sample_size: u64,
    pub scenario_set: String,
    pub target_metrics: BTreeMap<String, TargetMetricReport>,
    pub collateral_metrics: BTreeMap<String, CollateralMetricReport>,
    pub capability: CapabilityReport,
    pub isa_delta: f64,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict_notes: Option<String>,
    pub methodology: String,
    pub traces_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontier_record: Option<String>,
}

/// Report-level metadata supplied by the caller rather than computed.
#[derive(Debug, Clone)]
pub struct ReportMeta {
    pub satellite: String,
    pub evaluation_date: String,
    pub sample_size: u64,
    pub scenario_set: String,
    pub methodology: String,
    pub traces_available: bool,
    pub verdict_notes: Option<String>,
    pub frontier_record: Option<String>,
}

/// Assemble a `vexometer-efficacy-v2` report from an evaluation.
///
/// Returns the report plus any non-fatal warnings (currently: the D1d
/// singular-`frontier_record` ambiguity for multi-target reports).
pub fn build_report(
    eval: &Evaluation,
    meta: &ReportMeta,
) -> Result<(EfficacyReport, Vec<String>), EfficacyError> {
    let mut warnings = Vec::new();

    if eval.verdict == Verdict::AcceptWithWarning {
        let notes = meta.verdict_notes.as_deref().unwrap_or("");
        if notes.trim().is_empty() {
            return Err(EfficacyError::Data(format!(
                "verdict is accept_with_warning: the regressed metric(s) ({}) must be named \
                 in verdict_notes (pass --notes)",
                eval.warned_metrics.join(", ")
            )));
        }
        for m in &eval.warned_metrics {
            if !notes.contains(m.as_str()) {
                return Err(EfficacyError::Data(format!(
                    "verdict_notes must name regressed metric {m}"
                )));
            }
        }
    }

    if eval.targets.len() > 1 && meta.frontier_record.is_some() {
        warnings.push(format!(
            "frontier_record is a single reference but the report has {} targets; \
             plurality is undecided -- awaiting ruling D1d (see {ISSUE_D1})",
            eval.targets.len()
        ));
    }

    let target_metrics = eval
        .targets
        .iter()
        .map(|(m, t)| {
            (
                m.clone(),
                TargetMetricReport {
                    baseline: t.baseline,
                    after: t.after,
                    gap_closed: round3(t.gap_closed),
                    mean_reduction: round2(t.mean_reduction),
                    std_dev: t.std_dev,
                    confidence: t.confidence,
                    p_value: t.p_value,
                },
            )
        })
        .collect();

    let collateral_metrics = eval
        .collateral
        .iter()
        .map(|(m, c)| {
            (
                m.clone(),
                CollateralMetricReport {
                    baseline: c.baseline,
                    after: c.after,
                    delta: round2(c.delta),
                },
            )
        })
        .collect();

    let report = EfficacyReport {
        version: EFFICACY_VERSION.to_string(),
        satellite: meta.satellite.clone(),
        evaluation_date: meta.evaluation_date.clone(),
        sample_size: meta.sample_size,
        scenario_set: meta.scenario_set.clone(),
        target_metrics,
        collateral_metrics,
        capability: CapabilityReport {
            proxy: PROBE_PROXY_PATH.to_string(),
            probes_total: eval.capability.probes_total,
            pass_rate_before: round3(eval.capability.pass_rate_before),
            pass_rate_after: round3(eval.capability.pass_rate_after),
            capability_ok: eval.capability.capability_ok,
            probes_regressed: eval.capability.probes_regressed.clone(),
        },
        isa_delta: round2(eval.isa_delta_raw),
        verdict: eval.verdict,
        verdict_notes: meta.verdict_notes.clone(),
        methodology: meta.methodology.clone(),
        traces_available: meta.traces_available,
        frontier_record: meta.frontier_record.clone(),
    };
    Ok((report, warnings))
}

// ---------------------------------------------------------------------------
// vexometer-frontier-v1 records
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralMax {
    pub metric: String,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierAttempt {
    pub index: u64,
    pub satellite: String,
    pub config: String,
    pub target_after: f64,
    pub gap_closed: f64,
    pub collateral_max: CollateralMax,
    pub isa_delta: f64,
    pub capability_ok: bool,
    pub verdict: Verdict,
    pub frontier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierRecord {
    pub version: String,
    pub metric: String,
    pub model_profile: String,
    pub timestamp: String,
    pub scenario_set: String,
    pub baseline: BTreeMap<String, f64>,
    pub attempts: Vec<FrontierAttempt>,
    pub frontier_final: f64,
    pub methods_tried: u64,
    pub methods_rejected: u64,
}

impl FrontierRecord {
    pub fn new(
        metric: &str,
        model_profile: &str,
        timestamp: &str,
        scenario_set: &str,
        baseline_metric_score: f64,
        baseline_isa_score: f64,
        baseline_probe_pass_rate: f64,
    ) -> Self {
        let mut baseline = BTreeMap::new();
        baseline.insert(metric.to_string(), baseline_metric_score);
        baseline.insert("isa_score".to_string(), baseline_isa_score);
        baseline.insert(
            "probe_pass_rate".to_string(),
            round3(baseline_probe_pass_rate),
        );
        FrontierRecord {
            version: FRONTIER_VERSION.to_string(),
            metric: metric.to_string(),
            model_profile: model_profile.to_string(),
            timestamp: timestamp.to_string(),
            scenario_set: scenario_set.to_string(),
            baseline,
            attempts: Vec::new(),
            frontier_final: 0.0,
            methods_tried: 0,
            methods_rejected: 0,
        }
    }

    /// Append one attempt, enforcing the invariant: the frontier is
    /// monotonically non-decreasing and advances only on an `accept` or
    /// `accept_with_warning` whose `gap_closed` exceeds the current
    /// frontier.
    pub fn append(
        &mut self,
        satellite: &str,
        config: &str,
        eval: &Evaluation,
        scenario_set: &str,
    ) -> Result<&FrontierAttempt, EfficacyError> {
        if scenario_set != self.scenario_set {
            return Err(EfficacyError::Data(format!(
                "every attempt in a frontier record must run against the identical \
                 scenario set (record: {}, attempt: {})",
                self.scenario_set, scenario_set
            )));
        }
        let target = eval.targets.get(&self.metric).ok_or_else(|| {
            EfficacyError::Data(format!(
                "evaluation has no target outcome for this record's metric {}",
                self.metric
            ))
        })?;
        let (cmax_metric, cmax_delta) = eval
            .collateral_max()
            .ok_or_else(|| EfficacyError::Data("evaluation has no collateral metrics".into()))?;

        let prev = self.attempts.last().map(|a| a.frontier).unwrap_or(0.0);
        let gap = round3(target.gap_closed);
        let frontier = if eval.verdict.advances_frontier() && gap > prev {
            gap
        } else {
            prev
        };

        self.attempts.push(FrontierAttempt {
            index: self.attempts.len() as u64 + 1,
            satellite: satellite.to_string(),
            config: config.to_string(),
            target_after: target.after,
            gap_closed: gap,
            collateral_max: CollateralMax {
                metric: cmax_metric,
                delta: round2(cmax_delta),
            },
            isa_delta: round2(eval.isa_delta_raw),
            capability_ok: eval.capability.capability_ok,
            verdict: eval.verdict,
            frontier,
        });
        self.frontier_final = frontier;
        self.methods_tried = self.attempts.len() as u64;
        self.methods_rejected = self
            .attempts
            .iter()
            .filter(|a| a.verdict.is_reject())
            .count() as u64;
        Ok(self.attempts.last().unwrap())
    }
}

// ---------------------------------------------------------------------------
// Validation: recompute everything a stored report claims
// ---------------------------------------------------------------------------

fn known_metric(m: &str) -> bool {
    weight_of(m).is_some()
}

/// Validate a `vexometer-efficacy-v2` document. Returns a list of
/// problems; an empty list means the document is valid.
pub fn validate_efficacy(doc: &serde_json::Value) -> Vec<String> {
    let mut problems = Vec::new();
    let report: EfficacyReport = match serde_json::from_value(doc.clone()) {
        Ok(r) => r,
        Err(e) => return vec![format!("does not parse as {EFFICACY_VERSION}: {e}")],
    };

    if report.version != EFFICACY_VERSION {
        problems.push(format!(
            "version is {:?}, expected {EFFICACY_VERSION:?}",
            report.version
        ));
    }
    if !report.scenario_set.starts_with("sha256:") {
        problems.push("scenario_set is not content-addressed (sha256:...)".into());
    }
    if report.target_metrics.is_empty() {
        problems.push("target_metrics is empty".into());
    }

    // Coverage: targets and collateral must partition the ten metrics.
    for m in report.target_metrics.keys() {
        if !known_metric(m) {
            problems.push(format!("unknown target metric {m}"));
        }
        if report.collateral_metrics.contains_key(m) {
            problems.push(format!("{m} appears in both target and collateral sets"));
        }
    }
    for m in report.collateral_metrics.keys() {
        if !known_metric(m) {
            problems.push(format!("unknown collateral metric {m}"));
        }
    }
    for (m, _) in METRIC_WEIGHTS {
        if !report.target_metrics.contains_key(m) && !report.collateral_metrics.contains_key(m) {
            problems.push(format!(
                "metric {m} is in neither target_metrics nor collateral_metrics; \
                 collateral must cover every non-target metric"
            ));
        }
    }

    // Arithmetic: recompute each stored figure from its own raw values.
    for (m, t) in &report.target_metrics {
        if t.baseline == 0.0 {
            problems.push(format!(
                "target {m} has baseline 0: gap_closed is undefined (awaiting ruling D1a, {ISSUE_D1})"
            ));
            continue;
        }
        let gap = (t.baseline - t.after) / t.baseline;
        if (round3(gap) - t.gap_closed).abs() > 0.0005 + EPS {
            problems.push(format!(
                "target {m}: gap_closed {} does not match (baseline - after) / baseline = {}",
                t.gap_closed,
                round3(gap)
            ));
        }
        if (round2(t.baseline - t.after) - t.mean_reduction).abs() > 0.005 + EPS {
            problems.push(format!(
                "target {m}: mean_reduction {} does not match baseline - after = {}",
                t.mean_reduction,
                round2(t.baseline - t.after)
            ));
        }
    }
    for (m, c) in &report.collateral_metrics {
        if (round2(c.after - c.baseline) - c.delta).abs() > 0.005 + EPS {
            problems.push(format!(
                "collateral {m}: delta {} does not match after - baseline = {}",
                c.delta,
                round2(c.after - c.baseline)
            ));
        }
    }

    // D_ISA over all ten metrics with the METRICS.adoc weights.
    let mut num = 0.0;
    let mut den = 0.0;
    let mut complete = true;
    for (m, w) in METRIC_WEIGHTS {
        let (b, a) = if let Some(t) = report.target_metrics.get(m) {
            (t.baseline, t.after)
        } else if let Some(c) = report.collateral_metrics.get(m) {
            (c.baseline, c.after)
        } else {
            complete = false;
            continue;
        };
        num += w * (a - b);
        den += w;
    }
    if complete {
        let isa = round2(num / den * 100.0);
        if (isa - report.isa_delta).abs() > 0.005 + EPS {
            problems.push(format!(
                "isa_delta {} does not match weighted recomputation {}",
                report.isa_delta, isa
            ));
        }
    }

    // Capability gate consistency (aggregate form, per the normative table).
    if report.capability.probes_total == 0 {
        problems.push("capability.probes_total must be > 0".into());
    } else {
        let tol = 1.0 / f64::from(report.capability.probes_total);
        let ok =
            report.capability.pass_rate_after >= report.capability.pass_rate_before - tol - EPS;
        if ok != report.capability.capability_ok {
            problems.push(format!(
                "capability_ok is {} but pass rates {} -> {} with tolerance 1/{} imply {}",
                report.capability.capability_ok,
                report.capability.pass_rate_before,
                report.capability.pass_rate_after,
                report.capability.probes_total,
                ok
            ));
        }
    }

    // Verdict recomputation (skipped for the lifted-report sentinel).
    if report.verdict != Verdict::Unverified && problems.is_empty() {
        let improved: Vec<bool> = report
            .target_metrics
            .values()
            .map(|t| t.after < t.baseline - EPS)
            .collect();
        let all = improved.iter().all(|i| *i);
        let none = improved.iter().all(|i| !*i);
        if !all && !none {
            problems.push(format!(
                "targets disagree on improvement; the multi-target acceptance rule is \
                 undecided (awaiting ruling D1c, {ISSUE_D1})"
            ));
        } else {
            let max_c = report
                .collateral_metrics
                .values()
                .map(|c| c.delta)
                .fold(f64::NEG_INFINITY, f64::max);
            let warned: Vec<&String> = report
                .collateral_metrics
                .iter()
                .filter(|(_, c)| {
                    c.delta > COLLATERAL_ACCEPT + EPS && c.delta <= COLLATERAL_WARN + EPS
                })
                .map(|(m, _)| m)
                .collect();
            let expected = if none {
                Verdict::RejectNull
            } else if !report.capability.capability_ok {
                Verdict::RejectCapability
            } else if !report.collateral_metrics.is_empty() && max_c > COLLATERAL_WARN + EPS {
                Verdict::RejectCollateral
            } else if report.isa_delta >= 0.0 {
                Verdict::RejectNet
            } else if !warned.is_empty() {
                Verdict::AcceptWithWarning
            } else {
                Verdict::Accept
            };
            if expected != report.verdict {
                problems.push(format!(
                    "verdict is {} but the acceptance rule implies {}",
                    report.verdict.as_str(),
                    expected.as_str()
                ));
            }
            if report.verdict == Verdict::AcceptWithWarning {
                match &report.verdict_notes {
                    None => problems.push(
                        "accept_with_warning requires verdict_notes naming the regressed metric"
                            .into(),
                    ),
                    Some(notes) => {
                        for m in warned {
                            if !notes.contains(m.as_str()) {
                                problems
                                    .push(format!("verdict_notes must name regressed metric {m}"));
                            }
                        }
                    }
                }
            }
        }
    }

    problems
}

/// Validate a `vexometer-frontier-v1` document. Returns a list of
/// problems; an empty list means the document is valid.
pub fn validate_frontier(doc: &serde_json::Value) -> Vec<String> {
    let mut problems = Vec::new();
    let record: FrontierRecord = match serde_json::from_value(doc.clone()) {
        Ok(r) => r,
        Err(e) => return vec![format!("does not parse as {FRONTIER_VERSION}: {e}")],
    };

    if record.version != FRONTIER_VERSION {
        problems.push(format!(
            "version is {:?}, expected {FRONTIER_VERSION:?}",
            record.version
        ));
    }
    if !known_metric(&record.metric) {
        problems.push(format!("unknown metric {}", record.metric));
    }
    if !record.scenario_set.starts_with("sha256:") {
        problems.push("scenario_set is not content-addressed (sha256:...)".into());
    }
    for key in [record.metric.as_str(), "isa_score", "probe_pass_rate"] {
        if !record.baseline.contains_key(key) {
            problems.push(format!("baseline block is missing {key}"));
        }
    }

    let mut prev_frontier = 0.0;
    for (i, a) in record.attempts.iter().enumerate() {
        let expect_index = i as u64 + 1;
        if a.index != expect_index {
            problems.push(format!(
                "attempt {} has index {}, expected {expect_index}",
                i + 1,
                a.index
            ));
        }
        if !(0.0..=1.0).contains(&a.gap_closed) {
            problems.push(format!(
                "attempt {}: gap_closed {} outside [0, 1]",
                a.index, a.gap_closed
            ));
        }
        if a.verdict == Verdict::Unverified {
            problems.push(format!(
                "attempt {}: unverified is a verification-status sentinel, not a frontier verdict",
                a.index
            ));
        }
        let expected_frontier =
            if a.verdict.advances_frontier() && a.gap_closed > prev_frontier + EPS {
                a.gap_closed
            } else {
                prev_frontier
            };
        if (a.frontier - expected_frontier).abs() > EPS {
            problems.push(format!(
                "attempt {}: frontier {} violates the invariant (expected {}; the frontier \
                 advances only on accept/accept_with_warning exceeding the current frontier)",
                a.index, a.frontier, expected_frontier
            ));
        }
        if a.frontier < prev_frontier - EPS {
            problems.push(format!(
                "attempt {}: frontier decreased ({} -> {})",
                a.index, prev_frontier, a.frontier
            ));
        }
        prev_frontier = a.frontier;
    }

    if (record.frontier_final - prev_frontier).abs() > EPS {
        problems.push(format!(
            "frontier_final {} does not match last attempt frontier {}",
            record.frontier_final, prev_frontier
        ));
    }
    if record.methods_tried != record.attempts.len() as u64 {
        problems.push(format!(
            "methods_tried {} does not match attempt count {}",
            record.methods_tried,
            record.attempts.len()
        ));
    }
    let rejected = record
        .attempts
        .iter()
        .filter(|a| a.verdict.is_reject())
        .count() as u64;
    if record.methods_rejected != rejected {
        problems.push(format!(
            "methods_rejected {} does not match reject verdict count {rejected}",
            record.methods_rejected
        ));
    }

    problems
}
