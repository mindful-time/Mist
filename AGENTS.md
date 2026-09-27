# Repository instructions

## Smell review

- Run `uvx --from smells==0.5.0 smells check --path . --policy quality-policy.json --format table --log smells-findings.log --report smells-report.json` before reviewing or refactoring code smells.
- Treat exit code 2 as an incomplete scan, never as a clean result.
- Open `smells-findings.log`, start at its Issue Index, and read every referenced error, blocking, ignored, and review detail.
- Treat each finding as evidence to investigate, not proof that a defect exists.
- Before review, remediation, or suppression, open and read the exact reference URL and its `when_to_ignore` guidance.
- If the URL cannot be consulted, report the research as incomplete and stop review, remediation, and suppression for that finding.
- Verify the complete finding against its declaration, related locations, callers, tests, and repository contracts.
- Add `smells: ignore[exact-rule-id] -- non-empty reason` only when the verified exception applies to the next declaration; never add it merely to pass the hook.
- Prefer small behavior-preserving changes. Run the project's tests and rerun Smells after editing.
