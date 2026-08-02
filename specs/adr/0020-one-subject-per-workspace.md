# Keep exactly one Subject in each Workspace

Each Health Engine Workspace will contain the health record of exactly one Subject. The Operator using the Workspace may be that Subject, a caregiver, an agent, or an application, and operator provenance is recorded separately where relevant. Health Engine will not mix multiple subjects in one SQLite record or infer that the operator and subject are the same person. A caregiver managing several people uses a separate portable Workspace for each one.
