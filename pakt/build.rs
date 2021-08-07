use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Default)]
struct CmdMap {
    cmds: HashMap<String, CmdMap>,
}

impl CmdMap {
    fn subcmds(&self, path: &Path) -> Vec<&String> {
        let mut map: &CmdMap = self;
        for cmd in path.to_str().unwrap().split('/') {
            map = map.cmds.get(&cmd.to_string()).unwrap();
        }
        map.cmds()
    }

    fn cmds(&self) -> Vec<&String> {
        self.cmds.keys().collect()
    }
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let src_dir = env::current_dir().unwrap().join("src");
    let dest_path = Path::new(&out_dir);
    let subcmds_path = src_dir.join("subcmds");
    let mut cmd_map = CmdMap::default();

    let walker = WalkDir::new(&subcmds_path).into_iter();
    for entry in walker.filter_entry(|e| !is_hidden(e)) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let subcmd = path.strip_prefix(&subcmds_path).unwrap().with_extension("");
            if subcmd.file_name().is_some() {
                let cmd_path = subcmd.to_str().unwrap().to_string();
                let mut map: &mut CmdMap = &mut cmd_map;
                for cmd in cmd_path.split('/') {
                    map = map
                        .cmds
                        .entry(cmd.to_string())
                        .or_insert_with(CmdMap::default);
                }
            }
        }
    }

    let mut cmd_stack: Vec<(PathBuf, Vec<&String>)> = vec![(PathBuf::from(""), cmd_map.cmds())];

    while let Some((path, cmds)) = cmd_stack.pop() {
        let path_str = path.to_str().unwrap();
        let dir_path = format!("subcmds/{}", &path_str);
        let file_dir = dest_path.join(&dir_path);
        let file_path = match path_str {
            "" => "subcmds.rs".to_string(),
            _ => format!("{}.rs", &dir_path),
        };
        let src_module_path = src_dir.join(&file_path);
        let generated_module_path = dest_path.join(&file_path);

        fs::create_dir_all(&file_dir).unwrap();
        let file = fs::File::create(&generated_module_path).unwrap();

        // load subcommand modules from the src dir
        for s in &cmds {
            let module_path = src_dir.join(&dir_path).join(format!("{}.rs", &s));
            writeln!(&file, "#[path = {:?}]", module_path).unwrap();
            writeln!(&file, "mod {};", s).unwrap();
        }

        // auto-register subcommands for clap
        let cmd_strs = &cmds
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

        // build subcommand -> run function map
        let mut cmd_maps: Vec<String> = Vec::new();
        for s in &cmds {
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
        if !run_fn_exists(&src_module_path) {
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

        // inject nested subcommands for code generation
        for cmd in &cmds {
            let new_path = path.join(cmd);
            let subcmds = cmd_map.subcmds(&new_path);
            cmd_stack.push((new_path, subcmds));
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/subcmds");
}
