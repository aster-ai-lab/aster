# Feature Specification: [FEATURE NAME]

**Feature Branch**: `[###-feature-name]`

**Created**: [DATE]

**Status**: Draft

**Input**: User description: "$ARGUMENTS"

**Gate context**: [The owning app or crate and the slice this feature
serves. A feature with no roadmap row states why it exists anyway.]

## User Scenarios & Testing *(mandatory)*

### User Story 1 - [Brief Title] (Priority: P1)

[Describe this user journey in plain language]

**Why this priority**: [Explain the value and priority]

**Independent Test**: [The hermetic test that proves this story alone.
Name the package and the assertion. Do not require a live PostgreSQL
campaign unless this feature's store really has two adapters.]

**Acceptance Scenarios**:

1. **Given** [initial state], **When** [action], **Then** [expected outcome]

### User Story 2 - [Brief Title] (Priority: P2)

[Describe this user journey in plain language]

**Why this priority**: [Value and priority]

**Independent Test**: [Standalone test description]

**Acceptance Scenarios**:

1. **Given** [initial state], **When** [action], **Then** [expected outcome]

### Edge Cases

- What happens when [boundary condition]?
- How does the system behave under [error scenario]? Fail closed and
  name the refusal.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST [capability, including what it refuses]
- **FR-002**: System MUST refuse [the abuse case] with [the named reason]

### Key Entities *(include if the feature involves data)*

- **[Entity]**: [What it represents and which crate or app owns it]

## House Checklist *(mandatory — from Agents.md)*

- [ ] New behavior has a small hermetic test in the owning package
- [ ] Closed sets: every new enum has `parse()` / `as_str()`
- [ ] No production `unwrap()` / `expect()` without an infallibility comment
- [ ] Internal `a3s-*` dependencies stay on a path in this repository
- [ ] Product configuration, if any, is ACL parsed by `a3s-acl`
- [ ] Evidence target: `docs/evidence/<domain>/<date-slug>/OUTCOME.md`
      when the landing is recorded

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: [The hermetic suite and the count it must pass]

## Assumptions

- [Assumption]

## Explicitly Not Claimed

- [What this feature deliberately does not do]
