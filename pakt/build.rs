use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::Path;

use indoc::formatdoc;
use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let src_dir = env::current_dir().unwrap().join("src");
    let dest_path = Path::new(&out_dir);

    let mut subcmds: Vec<String> = Vec::new();
    let subcmds_path = src_dir.join("subcmds");
    let walker = WalkDir::new(&subcmds_path).into_iter();
    for entry in walker.filter_entry(|e| !is_hidden(e)) {
        let entry = entry.unwrap();
        let path = entry.path();
        let subcmd = path.strip_prefix(&subcmds_path).unwrap().with_extension("");
        if let Some(filename) = subcmd.file_name() {
            if filename.to_str().unwrap() != "mod" {
                subcmds.push(subcmd.to_str().unwrap().to_string());
            }
        }
    }

    let mut cmds: HashMap<String, Vec<String>> = HashMap::new();

    for subcmd in subcmds {
        let cmd: Vec<&str> = subcmd.split('/').collect();
        let key = cmd[..cmd.len() - 1].join("/");
        match cmds.get_mut(&key) {
            Some(vec) => vec.push(subcmd.to_string()),
            None => {
                cmds.insert(key, vec![subcmd.to_string()]);
            }
        }
    }

    for (level, subcmds) in cmds.iter() {
        let dir_path = format!("subcmds/{}", level);
        let file_dir = dest_path.join(&dir_path);
        let module_path = src_dir.join(&dir_path);
        fs::create_dir_all(&file_dir).unwrap();
        let file = fs::File::create(&file_dir.join("generated.rs")).unwrap();
        for cmd in subcmds {
            let module = cmd.split('/').last().unwrap();
            let module_path = match module_path.join(format!("{}.rs", module)).exists() {
                true => module_path.join(format!("{}.rs", module)),
                false => module_path.join(format!("{}/mod.rs", module)),
            };
            writeln!(&file, "#[path = \"{}\"]", module_path.to_str().unwrap()).unwrap();
            writeln!(&file, "mod {};", module).unwrap();
        }

        let cmd_strs = subcmds
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
        for s in subcmds {
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
    }

    println!("cargo:rerun-if-changed=build.rs");
}
