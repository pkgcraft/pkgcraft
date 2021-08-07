use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::Path;

use indoc::{formatdoc, indoc};
use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn run_fn_exists(path: &Path) -> bool {
    let src_contents = fs::read_to_string(&path).unwrap();
    for line in src_contents.lines() {
        if line.starts_with("pub fn run(") {
            return true;
        }
    }
    false
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let src_dir = env::current_dir().unwrap().join("src");
    let dest_path = Path::new(&out_dir);

    let mut subcmds: Vec<String> = Vec::new();
    let mut cmd_map: HashMap<String, Vec<String>> = HashMap::new();

    let subcmds_path = src_dir.join("subcmds");
    let walker = WalkDir::new(&subcmds_path).into_iter();
    for entry in walker.filter_entry(|e| !is_hidden(e)) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let subcmd = path.strip_prefix(&subcmds_path).unwrap().with_extension("");
            if subcmd.file_name().is_some() {
                let cmd_path = subcmd.to_str().unwrap().to_string();
                subcmds.push(cmd_path.clone());

                let cmd: Vec<String> = cmd_path.split('/').map(|s| s.to_string()).collect();
                let key = cmd[..cmd.len() - 1].join("/");
                match cmd_map.get_mut(&key) {
                    Some(vec) => vec.push(cmd_path),
                    None => {
                        cmd_map.insert(key, vec![cmd_path]);
                    }
                }
            }
        }
    }

    for (key, cmds) in cmd_map.iter() {
        let dir_path = format!("subcmds/{}", &key);
        let file_dir = dest_path.join(&dir_path);
        let module_file_path = match key.as_str() {
            "" => src_dir.join("subcmds.rs"),
            _ => src_dir.join(format!("{}.rs", dir_path)),
        };
        fs::create_dir_all(&file_dir).unwrap();
        let file = fs::File::create(&file_dir.join("generated.rs")).unwrap();
        for cmd in cmds {
            let module = cmd.split('/').last().unwrap();
            let module_path = src_dir.join(format!("subcmds/{}.rs", &cmd));
            writeln!(&file, "#[path = \"{}\"]", module_path.to_str().unwrap()).unwrap();
            writeln!(&file, "mod {};", module).unwrap();
        }

        let cmd_strs = cmds
            .iter()
            .map(|s| format!("{}::cmd()", s.split('/').last().unwrap()))
            .collect::<Vec<String>>()
            .join(", ");
        let register_func = formatdoc!(
            "

            pub fn register() -> Vec<clap::App<'static>> {{
                vec![{}]
            }}
        ",
            cmd_strs
        );
        write!(&file, "{}", register_func).unwrap();

        let mut cmd_maps: Vec<String> = Vec::new();
        for s in cmds {
            let cmd = s.split('/').last().unwrap();
            cmd_maps.push(format!("({:?}, {}::run as RunFn)", cmd, cmd));
        }

        let func_map = formatdoc!(
            "

            use std::collections::HashMap;

            use once_cell::sync::Lazy;

            type RunFn = fn(&ArgMatches, &mut Settings) -> Result<()>;

            static FUNC_MAP: Lazy<HashMap<&'static str, RunFn>> = Lazy::new(|| {{
                [
                    {}
                ].iter().cloned().collect()
            }});
        ",
            cmd_maps.join(",\n\t")
        );
        write!(&file, "{}", func_map).unwrap();

        // insert generic subcommand run function if none exists
        if !run_fn_exists(&module_file_path) {
            let run_fn = indoc! {"

                use anyhow::Result;
                use clap::ArgMatches;

                use crate::settings::Settings;

                pub fn run(args: &ArgMatches, settings: &mut Settings) -> Result<()> {
                    let (subcmd, m) = args.subcommand().unwrap();
                    let func = FUNC_MAP.get(subcmd).unwrap();
                    func(m, settings)
                }
            "};
            write!(&file, "{}", run_fn).unwrap();
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/subcmds");
}
