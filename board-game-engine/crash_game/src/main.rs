#![no_main]

extern crate alloc;

use crash_game::GameState;
use hyle_contract_sdk::guest::execute;
use hyle_contract_sdk::guest::GuestEnv;
use hyle_contract_sdk::guest::SP1Env;

sp1_zkvm::entrypoint!(main);

fn main() {
    let env = SP1Env {};
    let (commitment, input): (Vec<u8>, _) = env.read();
    let output = execute::<GameState>(&commitment, &input);
    env.commit(&output);
}
