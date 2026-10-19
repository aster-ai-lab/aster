# Aster local spine — https://docs.tilt.dev/
# Middleware: docker-compose.yml. Host processes: scripts/ensure-*.sh.
# Entry: `just up` → `tilt up`. Stop: `just down` → `tilt down` + host stops.
# This tree has api, auth, billing, config, search, web, and cloud.
# Gateway, the data-plane runtime, and parse are not started here.

update_settings(max_parallel_updates=4)

docker_compose('./docker-compose.yml')

# Wait for shared data plane before host ensure scripts.
dc_resource('a3s-postgres', labels=['middleware'])
dc_resource('a3s-redis', labels=['middleware'])
dc_resource('a3s-etcd', labels=['middleware'])
dc_resource('a3s-iggy', labels=['middleware'])
dc_resource('a3s-rustfs', labels=['middleware'])
dc_resource('a3s-registry', labels=['middleware'])
dc_resource('a3s-cloud-registry', labels=['middleware'])
dc_resource('a3s-clickhouse', labels=['middleware'])

middleware = [
    'a3s-postgres',
    'a3s-redis',
    'a3s-etcd',
    'a3s-iggy',
    'a3s-rustfs',
    'a3s-registry',
    'a3s-cloud-registry',
    'a3s-clickhouse',
]

# Postgres host helpers are source. ensure-* stops a live process only after
# they run. Leaving them out of deps made a script edit retrigger Tilt before
# the helper existed, and the listener stayed down (exit 127, no retry).
host_helpers = [
    'scripts/compose-postgres-host.sh',
    'scripts/resolve-container.sh',
]

local_resource(
    'shared-databases',
    cmd='bash scripts/ensure-shared-databases.sh',
    deps=[
        'scripts/ensure-shared-databases.sh',
        'scripts/host-pg-probe.py',
    ] + host_helpers,
    resource_deps=middleware,
    allow_parallel=True,
    labels=['spine'],
)

# Runtime files written by the ensure scripts are not source. Watching them
# makes Tilt restart the same resource forever.
local_resource(
    'config',
    cmd='bash scripts/ensure-config.sh',
    deps=['apps/config', 'scripts/ensure-config.sh', 'scripts/detach-service.sh'] + host_helpers,
    ignore=['apps/config/.data', 'apps/config/target'],
    resource_deps=['shared-databases'],
    allow_parallel=True,
    labels=['spine'],
)

local_resource(
    'api',
    cmd='bash scripts/ensure-api.sh',
    deps=['apps/api', 'scripts/ensure-api.sh', 'scripts/detach-service.sh'] + host_helpers,
    ignore=['apps/api/.data', 'apps/api/target', 'apps/api/dist'],
    resource_deps=['auth', 'config', 'a3s-redis', 'a3s-postgres', 'a3s-etcd'],
    allow_parallel=True,
    labels=['spine'],
)

# Cloud is best-effort (same as former `just up` leading `-`).
local_resource(
    'cloud',
    cmd='bash scripts/cloud-up.sh || echo "cloud best-effort: skipped or failed"',
    deps=['apps/cloud', 'scripts/cloud-up.sh'] + host_helpers,
    ignore=['apps/cloud/target', 'apps/cloud/.a3s'],
    resource_deps=['shared-databases', 'a3s-iggy', 'a3s-cloud-registry'],
    allow_parallel=True,
    labels=['control-plane'],
)

local_resource(
    'auth',
    cmd='bash scripts/ensure-auth.sh',
    deps=['apps/auth', 'scripts/ensure-auth.sh', 'scripts/detach-service.sh'] + host_helpers,
    ignore=['apps/auth/.data', 'apps/auth/target'],
    resource_deps=['shared-databases', 'a3s-etcd'],
    allow_parallel=True,
    labels=['spine'],
)

local_resource(
    'billing',
    cmd='bash scripts/ensure-billing.sh',
    deps=['apps/billing', 'scripts/ensure-billing.sh', 'scripts/detach-service.sh'] + host_helpers,
    ignore=['apps/billing/.data', 'apps/billing/target'],
    resource_deps=['shared-databases', 'a3s-etcd'],
    allow_parallel=True,
    labels=['spine'],
)

def tenant_slug_or_fail():
    raw = os.getenv('A3S_TENANT_SLUG', '')
    if raw == '':
        fail('A3S_TENANT_SLUG is required')
    if len(raw) > 32 or len(raw) < 1:
        fail('invalid A3S_TENANT_SLUG')
    first = raw[0]
    if first < 'a' or first > 'z':
        fail('invalid A3S_TENANT_SLUG')
    # Starlark strings are not iterable. Index them.
    for i in range(1, len(raw)):
        ch = raw[i]
        letter = ch >= 'a' and ch <= 'z'
        digit = ch >= '0' and ch <= '9'
        if not letter and not digit and ch != '-':
            fail('invalid A3S_TENANT_SLUG')
    return raw

# `tilt up` and `tilt ci` require the slug. `tilt down` does not start Console.
def tenant_slug_for_resource():
    sub = config.tilt_subcommand
    if sub == 'up' or sub == 'ci':
        return tenant_slug_or_fail()
    raw = os.getenv('A3S_TENANT_SLUG', '')
    if raw == '':
        return 'a3s'
    return raw

tenant_slug = tenant_slug_for_resource()

local_resource(
    'web',
    cmd='bash scripts/ensure-web.sh',
    env={
        'A3S_TENANT_SLUG': tenant_slug,
        'PUBLIC_A3S_TENANT_SLUG': tenant_slug,
    },
    deps=[
        'apps/web',
        'scripts/ensure-web.sh',
        'scripts/detach-service.sh',
        'scripts/check-tenant.ts',
        'scripts/tenant-fs.ts',
    ],
    ignore=['apps/web/.data', 'apps/web/.next', 'apps/web/dist', 'apps/web/node_modules'],
    resource_deps=['api'],
    allow_parallel=True,
    labels=['spine'],
)

local_resource(
    'search',
    cmd='bash scripts/ensure-search.sh',
    deps=['crates/search', 'scripts/ensure-search.sh', 'scripts/detach-service.sh'],
    ignore=['crates/search/.data', 'crates/search/target'],
    allow_parallel=True,
    labels=['spine'],
)

local_resource(
    'spine-status',
    cmd='bash scripts/spine-status.sh',
    deps=['scripts/spine-status.sh'],
    resource_deps=['config', 'api', 'auth', 'billing', 'web', 'search'],
    labels=['spine'],
)
