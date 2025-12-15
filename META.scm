;; META.scm - Vexometer Meta-Information
;; SPDX-License-Identifier: AGPL-3.0-or-later
;; SPDX-FileCopyrightText: 2025 Jonathan D.A. Jewell

(meta
  (project "vexometer")
  (full-name "Vexometer: Irritation Surface Analyser")
  (tagline "Measures AI assistant irritation surfaces. Doesn't fix them.")

  (classification
    (domain "ai-tooling")
    (type "diagnostic-instrument")
    (maturity "alpha"))

  (philosophy
    (principle "measure-not-fix"
      "Vexometer diagnoses; it does not prescribe treatment")
    (principle "lower-is-better"
      "All metrics normalised 0-1 where 0 is ideal")
    (principle "satellite-independence"
      "Intervention tools work standalone, vexometer integration optional")
    (principle "formal-verification"
      "Metric calculations in SPARK subset where possible"))

  (metrics-overview
    ;; Original 6 metrics (v1)
    (metric TII "Temporal Intrusion Index"
      "Time-wasting behaviours, unnecessary delays")
    (metric LPS "Linguistic Pathology Score"
      "Verbal tics, padding, sycophancy, filler")
    (metric EFR "Epistemic Failure Rate"
      "Hallucination, false confidence, fabrication")
    (metric PQ "Paternalism Quotient"
      "Over-helping, unsolicited warnings, lecturing")
    (metric TAI "Telemetry Anxiety Index"
      "Privacy concerns, surveillance behaviours")
    (metric ICS "Interaction Coherence Score"
      "Conversation flow, consistency, coherence")
    ;; Extended 4 metrics (v2)
    (metric CII "Completion Integrity Index"
      "Incomplete outputs, placeholders, lazy generation")
    (metric SRS "Strategic Rigidity Score"
      "Backtrack resistance, sunk-cost patching")
    (metric SFR "Scope Fidelity Ratio"
      "Scope creep/collapse, request alignment")
    (metric RCI "Recovery Competence Index"
      "Error recovery quality, strategy variation"))

  (technical
    (primary-language "Ada/SPARK")
    (secondary-languages ("Rust" "Elixir"))
    (build-system "Alire + Nix")
    (automation "justfile")
    (formal-verification "SPARK subset for metric calculations")
    (ci-cd "GitLab CI + GitHub Actions mirror"))

  (governance
    (license "AGPL-3.0-or-later")
    (standard "RSR (Rhodium Standard Repository)")
    (maintainer "Jonathan D.A. Jewell")
    (contribution-model "cathedral"))

  (architecture-decisions
    (adr-001
      (title "RSR Compliance")
      (status "accepted")
      (date "2024-12")
      (decision "Follow Rhodium Standard Repository guidelines")
      (consequences ("RSR Gold target" "SHA-pinned actions" "SPDX headers")))
    (adr-002
      (title "Satellite Architecture")
      (status "accepted")
      (date "2025-01")
      (decision "Keep vexometer as pure diagnostic; interventions in satellites")
      (consequences ("Clean separation of concerns" "Independent development"
                     "Optional integration" "Avoid scope creep")))
    (adr-003
      (title "Metric Normalisation")
      (status "accepted")
      (date "2025-01")
      (decision "All metrics 0-1 scale where lower is better")
      (consequences ("Consistent interpretation" "Easy aggregation"
                     "ISA score is weighted sum")))))
