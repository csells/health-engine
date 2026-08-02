# Mark saved Analyses stale when their inputs change

Every saved Analysis will record the Health Facts and Corrections, public reference-data versions, curated knowledge version, confidence rules, and engine version that produced it. When any dependency changes, Health Engine marks that Analysis stale and will not present it as the current health picture until a new Analysis is computed. Historical Analyses remain available for audit and comparison; they are never silently rewritten or mistaken for current conclusions.
