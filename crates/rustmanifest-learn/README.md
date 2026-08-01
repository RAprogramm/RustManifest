<!--
SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
SPDX-License-Identifier: MIT
-->

# rustmanifest-learn

Interpretable learning layer over the engine's behavioral fact base.

Trains one Tsetlin machine per behavioral role on the engine's binarized
feature vectors and dumps what it learned as an auditable list of
conjunctive clauses (`IF effect-net AND NOT writes-own-field`). Training
is fully deterministic: fixed seed, sequential updates — same sources,
same clauses.

Admitted behind the research-doc gate: the prototype beat logistic
regression on identical features by +17pp macro-F1 (threshold 5pp).
Bootstrap labels come from the engine's deterministic role heuristic;
learned clauses are a lens on the codebase, not a source of findings.
