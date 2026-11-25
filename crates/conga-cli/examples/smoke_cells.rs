//! Throughput probe across a handful of cells to see how matches/sec
//! varies with starting score. One-off, not part of slice 2's
//! deliverable. Run with
//! `cargo run -p conga-cli --release --example smoke_cells`.

use std::time::Instant;

use conga_cli::build_equity::compute_cell;

fn main() {
    let n = 300;
    let seed = 1;
    let cells: &[(i32, i32)] = &[
        (-50, -50),
        (-30, 0),
        (0, 0),
        (20, 60),
        (50, 50),
        (80, 80),
        (0, 90),
        (90, 0),
    ];
    for &(a, b) in cells {
        let start = Instant::now();
        let p = compute_cell(a, b, n, seed);
        let el = start.elapsed().as_secs_f64();
        let ms = el * 1000.0 / n as f64;
        let mps = n as f64 / el;
        println!(
            "cell ({a:>3},{b:>3})  N={n}  p={p:.3}  elapsed={el:6.2}s  ms/match={ms:5.1}  matches/sec={mps:6.1}"
        );
    }
}
