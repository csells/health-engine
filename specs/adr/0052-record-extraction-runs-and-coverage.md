# Record Extraction Runs and their coverage

Every document-extraction attempt will identify the extractor and version, Source, declared regions and domains examined, omissions, failures, and resulting candidates as an Extraction Run. Atomicity applies to one Record Change Set produced by a run rather than permanently closing a Source after one pass; multiple improved runs may revisit the same Source, and their coverage is what lets Health Engine distinguish `not found` from `not looked for` without asking the Subject to adjudicate extraction quality.
