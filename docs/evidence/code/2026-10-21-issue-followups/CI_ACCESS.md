# CI source access

The legacy A3S Code workflow builds its own public A3S-Lab/Code repository and checks out public A3S-Lab/ACL at an explicit revision for its pager. Its setup-workspace.sh removes monorepo patches for standalone builds; it does not provide private Work checkout credentials. The local old OS workflow only builds sandbox-engine artifacts, and the pinned old Work source has no tracked .github workflow. A local SSH submodule checkout does not give GitHub-hosted runners those credentials.

Aster's root workspace parses Work and uses its Office vendor source even for the CLI. Work is aster-ai-lab/work; the separate backend workspaces are kense-labs/api and kense-labs/cloud. Each checkout resolves the exact recorded root gitlink; it never follows the source default branch.

The owner setup is a dedicated GitHub App installed for only those three repositories, with Contents:read (plus GitHub's required Metadata:read). Save its App ID and private key as CI_SOURCE_APP_ID and CI_SOURCE_APP_PRIVATE_KEY in Aster Actions secrets. Do not put the key in chat or source. The composite action further narrows each token to one repository and revokes it after the job.

Repository-level secret inventories for both kense-labs/os and aster-ai-lab/aster were empty at inspection. Organization secret inventory returned HTTP 403; this does not prove that organization secrets are absent. Work deploy-key creation returned the explicit repository-policy refusal HTTP 422. Neither policy nor permission scopes were changed.

References: [legacy Code workflow](https://github.com/A3S-Lab/Code/blob/cfaa22e8f1fca7ad1bfe132ac97ce7be930071f2/.github/workflows/ci.yml), [GitHub App token action](https://github.com/actions/create-github-app-token), [Aster Actions secrets](https://github.com/aster-ai-lab/aster/settings/secrets/actions).
