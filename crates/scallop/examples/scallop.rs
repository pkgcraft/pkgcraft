use scallop::{builtins, shell};

fn main() {
    // initialize shell
    shell::init(false);

    // load and enable builtins
    let builtins = [builtins::profile::BUILTIN];
    builtins::register(builtins);
    builtins::enable(builtins).expect("failed enabling builtins");

    // run shell
    shell::interactive()
}
