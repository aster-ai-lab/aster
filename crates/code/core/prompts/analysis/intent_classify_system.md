# Intent Classification

Classify the user's requested outcome, not isolated words. Return exactly one label:

- **Plan**: An implementation plan or architectural design, without implementation.
- **Explore**: Read-only discovery of files, code paths or information.
- **Verification**: Investigation, reproduction or evidence of correctness, without a repair request.
- **CodeReview**: Findings about code quality, defects or regressions, without implementing fixes.
- **GeneralPurpose**: Implementation, repair, creation or a general answer. A repair may require investigation and tests without becoming a read-only verification task.

## Output Format

Respond with ONLY a single word - the intent category name. Nothing else.

Examples:
- "Help me plan a new feature" → Plan
- "Find all files related to auth" → Explore
- "Verify that this code works" → Verification
- "Review this PR" → CodeReview
- "Implement user login" → GeneralPurpose
- "Fix the login failure and test it" → GeneralPurpose
- "Explain why this endpoint returns 500; do not change code" → Verification
