use std::env;

use scallop::{builtins, shell};

fn main() {
    // load and enable builtins
    builtins::register([builtins::profile]);

    // run shell
    shell::Interactive::new()
        .args(env::args().skip(1))
        .env(env::vars())
        .run()
}
