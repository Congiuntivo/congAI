//! Dump a handful of committed-table equity values so the slice-3
//! stop report can show what the Monte Carlo produced.

use conga_ai::committed_table;

fn main() {
    let t = committed_table();
    let samples = [
        (0, 0),
        (-20, 20),
        (20, -20),
        (-40, 60),
        (50, 50),
        (80, 20),
        (20, 80),
        (0, 90),
        (90, 0),
        (-50, 100),
        (100, -50),
        (0, 50),
        (50, 0),
    ];
    for (a, b) in samples {
        let v = t.lookup(a, b);
        println!("equity(own={a:>4}, opp={b:>4}) = {v:.4}");
    }
}
