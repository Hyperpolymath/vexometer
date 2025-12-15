;; ECOSYSTEM.scm - Vexometer Ecosystem Relationships
;; SPDX-License-Identifier: AGPL-3.0-or-later
;; SPDX-FileCopyrightText: 2025 Jonathan D.A. Jewell
;; This is the HUB of the vexometer ecosystem

(ecosystem
  (project "vexometer")
  (role "diagnostic-instrument")
  (ecosystem-position "hub")

  (provides
    (capability "irritation-surface-measurement")
    (capability "isa-scoring")
    (capability "model-comparison")
    (interface "metric-definitions")
    (interface "trace-format")
    (interface "validation-protocol"))

  (consumes
    ;; Vexometer consumes nothing - it's the root diagnostic
    )

  (satellites
    ;; Intervention repos that reduce specific metrics
    ;; Each is independent but validates against vexometer

    (satellite
      (name "vex-lazy-eliminator")
      (reduces (CII LPS))
      (status "planned")
      (description "Completeness enforcement for LLM outputs"))

    (satellite
      (name "vex-hallucination-guard")
      (reduces (EFR))
      (status "planned")
      (description "Verification layer for LLM factual claims"))

    (satellite
      (name "vex-sycophancy-shield")
      (reduces (LPS EFR))
      (status "planned")
      (description "Epistemic commitment tracking for LLMs"))

    (satellite
      (name "vex-confidence-calibrator")
      (reduces (EFR))
      (status "planned")
      (description "Structured uncertainty for LLM outputs"))

    (satellite
      (name "vex-specification-anchor")
      (reduces (SFR ICS))
      (status "planned")
      (description "Immutable requirements ledger"))

    (satellite
      (name "vex-instruction-persistence")
      (reduces (TII ICS))
      (status "planned")
      (description "System instruction compliance enforcement"))

    (satellite
      (name "vex-backtrack-enabler")
      (reduces (SRS ICS))
      (status "planned")
      (description "Low-friction restart support"))

    (satellite
      (name "vex-context-firewall")
      (reduces (EFR ICS))
      (status "planned")
      (description "Truth maintenance for conversation context"))

    (satellite
      (name "vex-scope-governor")
      (reduces (SFR PQ))
      (status "planned")
      (description "Scope contract enforcement"))

    (satellite
      (name "vex-error-recovery")
      (reduces (RCI))
      (status "planned")
      (description "Strategy variation on failure"))

    (satellite
      (name "vex-verbosity-compressor")
      (reduces (LPS TII))
      (status "planned")
      (description "Information density optimisation"))

    (satellite
      (name "vex-clarification-gate")
      (reduces (PQ TII))
      (status "planned")
      (description "Risk-weighted ambiguity handling")))

  (integration-points
    (point
      (name "trace-validation")
      (description "Satellites can include before/after traces validated by vexometer")
      (protocol "vexometer-trace-v1"))

    (point
      (name "efficacy-reporting")
      (description "Satellites report metric reduction percentages")
      (protocol "vexometer-efficacy-v1"))

    (point
      (name "metric-subscription")
      (description "Satellites can subscribe to specific metric calculations")
      (protocol "vexometer-metrics-v1")))

  (related-projects
    (project
      (name "rhodium-standard-repositories")
      (url "https://github.com/hyperpolymath/rhodium-standard-repositories")
      (relationship "standard"))
    (project
      (name "vexometer-satellites")
      (url "https://gitlab.com/hyperpolymath/vexometer-satellites")
      (relationship "index"))))
