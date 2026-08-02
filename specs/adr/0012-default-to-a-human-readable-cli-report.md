# Default the CLI to a human-readable report

Running `health-engine analysis run` without an output-format option will write a readable terminal report to stdout. Agents and applications request the complete structured contract with `--format json`; users may request a portable report with `--format markdown`. All formats are rendered from the same Analysis and contain no independently generated clinical meaning. Progress and diagnostics go to stderr so JSON and other stdout output remain clean and safe to pipe.
