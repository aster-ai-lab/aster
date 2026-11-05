use super::tests::{limits, policy};
use super::*;

#[test]
fn credential_concurrency_spans_routes_and_keeps_other_credentials_independent() {
    let (mut policy, identity) = policy(limits(100, 1000, 1000));
    policy
        .credentials
        .get_mut(&identity.credential_id)
        .unwrap()
        .max_concurrent_requests = std::num::NonZeroU32::new(1);
    let second_credential = Uuid::new_v4();
    let mut credential = policy.credentials[&identity.credential_id].clone();
    credential.credential_id = second_credential;
    policy.credentials.insert(second_credential, credential);
    let mut route = policy.routes[&identity.route_id].clone();
    route.route_id = Uuid::new_v4();
    route.grants.insert(
        second_credential,
        route.grants[&identity.credential_id].clone(),
    );
    let other_route = InferenceGrantIdentity {
        route_id: route.route_id,
        ..identity
    };
    policy.routes.insert(route.route_id, route);
    let store = InferenceLimitStore::new(&policy, None);
    let first = store
        .try_admit(identity, TokenReservation::default(), None)
        .unwrap();
    assert!(matches!(
        store.try_admit(other_route, TokenReservation::default(), None),
        Err(InferenceAccessError::ConcurrencyLimited)
    ));
    let independent = store
        .try_admit(
            InferenceGrantIdentity {
                credential_id: second_credential,
                ..other_route
            },
            TokenReservation::default(),
            None,
        )
        .unwrap();
    drop(first);
    assert!(store
        .try_admit(other_route, TokenReservation::default(), None)
        .is_ok());
    drop(independent);
}

#[test]
fn live_cap_includes_uncapped_requests_across_generation_and_policy_changes() {
    let (mut policy, identity) = policy(limits(100, 1000, 1000));
    let store = InferenceLimitStore::new(&policy, None);
    let first = store
        .try_admit(identity, TokenReservation::default(), None)
        .unwrap();
    let second = store
        .try_admit(identity, TokenReservation::default(), None)
        .unwrap();
    let credential = policy.credentials.get_mut(&identity.credential_id).unwrap();
    credential.max_concurrent_requests = std::num::NonZeroU32::new(1);
    credential.generation += 1;
    let route = policy.routes.get_mut(&identity.route_id).unwrap();
    route.policy_revision += 1;
    route
        .grants
        .get_mut(&identity.credential_id)
        .unwrap()
        .credential_generation += 1;
    let next_identity = InferenceGrantIdentity {
        policy_revision: identity.policy_revision + 1,
        credential_generation: identity.credential_generation + 1,
        ..identity
    };
    let next = InferenceLimitStore::new(&policy, Some(&store));
    assert!(matches!(
        next.try_admit(next_identity, TokenReservation::default(), None),
        Err(InferenceAccessError::ConcurrencyLimited)
    ));
    drop(first);
    assert!(matches!(
        next.try_admit(next_identity, TokenReservation::default(), None),
        Err(InferenceAccessError::ConcurrencyLimited)
    ));
    drop(second);
    assert!(next
        .try_admit(next_identity, TokenReservation::default(), None)
        .is_ok());
}

#[test]
fn failed_grant_admission_does_not_leak_a_credential_permit() {
    let (mut policy, identity) = policy(limits(100, 1, 1));
    policy
        .credentials
        .get_mut(&identity.credential_id)
        .unwrap()
        .max_concurrent_requests = std::num::NonZeroU32::new(1);
    let mut route = policy.routes[&identity.route_id].clone();
    route.route_id = Uuid::new_v4();
    let other_route = InferenceGrantIdentity {
        route_id: route.route_id,
        ..identity
    };
    policy.routes.insert(route.route_id, route);
    let store = InferenceLimitStore::new(&policy, None);
    drop(
        store
            .try_admit(identity, TokenReservation::default(), None)
            .unwrap(),
    );
    assert!(matches!(
        store.try_admit(identity, TokenReservation::default(), None),
        Err(InferenceAccessError::RateLimited { .. })
    ));
    assert!(store
        .try_admit(other_route, TokenReservation::default(), None)
        .is_ok());
}
