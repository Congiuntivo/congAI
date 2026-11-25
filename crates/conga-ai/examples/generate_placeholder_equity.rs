//! One-off generator for the slice-1 placeholder equity artefact.
//!
//! Writes `assets/equity_2p.json` with the analytical placeholder
//! values. Slice 3's `conga-cli build-equity` will overwrite the same
//! file with real Monte Carlo values and this example will become
//! historical.
//!
//! Usage: `cargo run -p conga-ai --example generate_placeholder_equity`

use conga_ai::EquityTable;
use conga_ai::equity::COMMITTED_ASSET_PATH;

fn main() {
    let table = EquityTable::placeholder();
    let json = table
        .to_json_string_pretty()
        .expect("serialize placeholder equity table");
    std::fs::write(COMMITTED_ASSET_PATH, json).expect("write committed equity_2p.json");
    println!(
        "wrote {} cells to {}",
        table.values.len(),
        COMMITTED_ASSET_PATH
    );
}
