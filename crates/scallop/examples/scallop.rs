use std::env;

use scallop::{builtins, shell};

fn main() {
    // initialize shell
    shell::init();

    // load and enable builtins
    let builtins = [builtins::profile::BUILTIN];
    builtins::register(builtins);
    builtins::enable(builtins).expect("failed enabling builtins");

    // run shell
    shell::Interactive::new()
        .args(env::args().skip(1))
        .env(env::vars())
        .run()
}
