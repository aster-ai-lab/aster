#!/usr/bin/env bash
set -euo pipefail

: "${A3S_CLOUD_TEST_POSTGRES_URL:?Set the live PostgreSQL proof URL explicitly}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
proof_dir="${1:-$(mktemp -d "${TMPDIR:-/tmp}/a3s-team-authorization.XXXXXX")}"
mkdir -p "$proof_dir"
proof_dir="$(cd "$proof_dir" && pwd)"

rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/api/Cargo.toml" \
  api_migration_manifest_is_contiguous_and_includes_explicit_role_scope -- --test-threads=1 \
  > "$proof_dir/migration-manifest.log" 2>&1
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/api/Cargo.toml" \
  contexts::member:: -- --test-threads=1 > "$proof_dir/member-tests.log" 2>&1
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/api/Cargo.toml" \
  contexts::work_inbox:: -- --test-threads=1 > "$proof_dir/work-inbox-tests.log" 2>&1
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/api/Cargo.toml" \
  contexts::schedule:: -- --test-threads=1 > "$proof_dir/schedule-tests.log" 2>&1
# The legacy mail fixture mutates DATABASE_URL; the owned PG authority campaign below proves team mail.
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/api/Cargo.toml" \
  contexts::mail:: -- --test-threads=1 --skip postgres_member_mail_assignment_and_delivery \
  > "$proof_dir/mail-tests.log" 2>&1
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/api/Cargo.toml" \
  postgres_role_policy_persistence_and_revocation -- --ignored --test-threads=1 \
  > "$proof_dir/postgres-role-policy.log" 2>&1
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/api/Cargo.toml" \
  postgres_directory_authority_campaign -- --ignored --test-threads=1 \
  > "$proof_dir/postgres-directory-authority.log" 2>&1
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/studio/src-tauri/Cargo.toml" \
  --lib agent_plane::mail_mcp -- --test-threads=1 > "$proof_dir/native-mail-mcp.log" 2>&1
rustup run 1.97.1 cargo test --manifest-path "$repo_root/apps/studio/src-tauri/Cargo.toml" \
  --lib org_login::role_history_tests -- --test-threads=1 > "$proof_dir/native-role-history.log" 2>&1
(
  cd "$repo_root/apps/studio"
  ./node_modules/.bin/vitest run --project renderer --maxWorkers 1 \
    src/renderer/pages/org/__tests__/OrgAuthorization.test.tsx \
    src/renderer/pages/org/__tests__/OrgPage.test.tsx \
    src/renderer/pages/org/__tests__/orgApi.test.ts \
    src/renderer/pages/org/__tests__/OrgWorkbench.test.tsx \
    src/renderer/components/workInbox/WorkInboxPopup.test.tsx \
    src/renderer/workInbox/client.test.ts \
    src/renderer/data/memberOrgs.test.ts \
    src/renderer/pages/schedule/__tests__/ScheduleAuthorization.test.tsx \
    src/renderer/pages/schedule/__tests__/SchedulePage.test.tsx \
    src/renderer/pages/schedule/__tests__/scheduleApi.test.ts \
    src/renderer/pages/mail/__tests__/MailComposeSurface.test.tsx \
    src/renderer/pages/mail/__tests__/MailAuthorization.test.tsx \
    src/renderer/pages/mail/__tests__/MailPage.test.tsx \
    src/renderer/mail/mailApi.test.ts \
    src/renderer/pages/mail/mailToBot.test.ts
) > "$proof_dir/renderer-tests.log" 2>&1
printf 'Member, memory/live PostgreSQL and renderer boundary checks passed. Evidence: %s\n' "$proof_dir"
