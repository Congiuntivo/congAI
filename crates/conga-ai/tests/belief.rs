//! Integration test for `BeliefTracker`.
//!
//! Drives a 2-player hand with uniform-random actions, feeds every event
//! into a tracker watching from seat 0, and at each Draw/Discard turn
//! samples 20 states and verifies:
//! - `rules.observe(sample, seat 0) == obs` (D32 + conformance test 5)
//! - each opponent's sampled hand ⊇ `known_held[opponent]` (D32)
//!
//! Marked `#[ignore]`; run with `--release --ignored`.

use conga_ai::BeliefTracker;
use conga_core::{ActionList, Event, Phase, RandomChance, Rng, RuleSet, Seat};
use conga_rules::StandardConga;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

#[test]
#[ignore = "slow (200 hands x ~20 sample bursts; ~3-5s release); run with --release --ignored"]
fn belief_tracker_consistent_over_200_hands() {
    let rules = StandardConga::new();
    let mut chance = RandomChance::new(ChaCha8Rng::seed_from_u64(0xBEEF));
    let mut sampling_rng = ChaCha8Rng::seed_from_u64(0xA1A1);

    for hand_idx in 0..200u32 {
        let scores = vec![0i32; 2];
        let mut state = rules.new_hand(&scores, Seat::new(0), &mut chance);
        let mut tracker = BeliefTracker::new(Seat::new(0), 2);
        let mut legal = ActionList::new();

        loop {
            if let Some(result) = rules.hand_result(&state) {
                tracker.notify(&Event::HandEnded(result));
                break;
            }

            let obs = rules.observe(&state, Seat::new(0));
            if matches!(obs.phase, Phase::Draw | Phase::Discard) {
                for _ in 0..20 {
                    let sample = tracker.sample_state(&mut sampling_rng, &obs, &rules);
                    assert_eq!(
                        rules.observe(&sample, Seat::new(0)),
                        obs,
                        "hand {hand_idx}: sampled state does not reproduce observation"
                    );
                    for opp in 0..2u8 {
                        let s = Seat::new(opp);
                        if s == Seat::new(0) {
                            continue;
                        }
                        let Some(known) = tracker.known_held(s) else {
                            continue;
                        };
                        for &c in known {
                            assert!(
                                sample.hands[opp as usize].contains(c),
                                "hand {hand_idx}: known card {c} missing from opponent's sampled hand"
                            );
                        }
                    }
                }
            }

            legal.clear();
            rules.legal_actions(&state, &mut legal);
            assert!(!legal.is_empty(), "running hand without a legal action");
            let idx = (chance.rng.next_u32() as usize) % legal.len();
            let action = legal[idx].clone();

            let mut events: Vec<Event> = Vec::new();
            rules.apply(&mut state, action, &mut chance, &mut events);
            for e in &events {
                tracker.notify(e);
            }
        }
    }
}
