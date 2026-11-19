//! Conformance test 5 for `StandardConga`: every sampled state must
//! reproduce the observation it was sampled from.
//!
//! Marked `#[ignore]` so default `cargo test --workspace` stays fast;
//! run with `cargo test -p conga-rules --test sample_state --release
//! -- --ignored`.

use conga_core::conformance::run_sample_state_checks;
use conga_rules::StandardConga;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

#[test]
#[ignore = "slow (500 hands x 20 samples, ~1-2s release); run with --release --ignored"]
fn standard_conga_sample_state_round_trips_over_500_hands() {
    let rules = StandardConga::new();
    let mut rng = ChaCha8Rng::seed_from_u64(0xC0FFEE);
    run_sample_state_checks(&rules, &mut rng, 500, 20);
}
