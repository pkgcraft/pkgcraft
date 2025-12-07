use std::{env, fs};

use bindgen::callbacks::{ItemInfo, ParseCallbacks};
use camino::Utf8Path;
use regex::Regex;

#[derive(Debug)]
struct BashCallback;

// rename bash data structures for consistency
impl ParseCallbacks for BashCallback {
    fn item_name(&self, item_info: ItemInfo) -> Option<String> {
        match item_info.name {
            // structs
            "word_desc" | "WORD_DESC" => Some("WordDesc".into()),
            "word_list" | "WORD_LIST" => Some("WordList".into()),
            "SHELL_VAR" => Some("ShellVar".into()),
            "ARRAY" => Some("Array".into()),
            "command" => Some("Command".into()),
            "builtin" => Some("Builtin".into()),
            // global mutables
            "global_command" => Some("GLOBAL_COMMAND".into()),
            "this_command_name" => Some("CURRENT_COMMAND_NAME".into()),
            "this_shell_builtin" => Some("CURRENT_BUILTIN_FUNC".into()),
            "current_builtin" => Some("CURRENT_BUILTIN".into()),
            "temporary_env" => Some("TEMPORARY_ENV".into()),
            "ifs_value" => Some("IFS".into()),
            "shell_builtins" => Some("SHELL_BUILTINS".into()),
            "num_shell_builtins" => Some("NUM_SHELL_BUILTINS".into()),
            "subshell_level" => Some("SUBSHELL_LEVEL".into()),
            "restricted" => Some("RESTRICTED".into()),
            "restricted_shell" => Some("RESTRICTED_SHELL".into()),
            "shell_pgrp" => Some("SHELL_PID".into()),
            // global constants
            "dist_version" => Some("DIST_VERSION".into()),
            "patch_level" => Some("PATCH_LEVEL".into()),
            // functions
            "get_minus_o_opts" => Some("get_set_options".into()),
            _ => None,
        }
    }
}

fn main() {
    // export SCALLOP_NO_VENDOR=1 to force system libscallop usage
    println!("cargo:rerun-if-env-changed=SCALLOP_NO_VENDOR");
    let forced_no_vendor = env::var_os("SCALLOP_NO_VENDOR").is_some_and(|s| s != "0");

    let vendor_dir = &Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("bash");
    let out_dir = &Utf8Path::new(&env::var("OUT_DIR").unwrap()).join("bash");
    fs::create_dir_all(out_dir).unwrap();

    let mut bindings = bindgen::Builder::default();

    // determine shared or static library usage
    if forced_no_vendor {
        // determine bundled bash version
        let data = fs::read_to_string(vendor_dir.join("configure")).unwrap();
        let re = Regex::new(r"(?m)^PACKAGE_VERSION='(?<version>.+)'$").unwrap();
        let caps = re.captures(&data).unwrap();
        let version = &caps["version"];

        // use pkg-config to determine header paths and verify version
        match pkg_config::Config::new()
            .exactly_version(version)
            .probe("scallop")
        {
            Ok(lib) => {
                // add include paths to header search path
                for path in &lib.include_paths {
                    bindings = bindings.clang_arg(format!("-I{}", path.display()));
                }
            }
            Err(_) => panic!("unable to find system library: scallop-{version}"),
        }

        // link using shared library
        println!("cargo::rustc-link-lib=dylib=scallop");
    } else {
        let mut cfg = autotools::Config::new(vendor_dir);
        cfg.make_args(vec![format!("-j{}", num_cpus::get())]);
        cfg.make_target("libscallop.a");
        cfg.out_dir(out_dir);

        // load required configure options
        let options =
            fs::read_to_string(vendor_dir.join("configure-scallop-options")).unwrap();
        let options = options
            .lines()
            .filter(|x| !x.is_empty() && !x.starts_with('#'));
        let re = Regex::new(r"^--(?<option>.+)$").unwrap();
        for option in options {
            if let Some(caps) = re.captures(option) {
                cfg.config_option(&caps["option"], None);
            } else {
                panic!("invalid configure option: {option}");
            }
        }

        // build static library
        cfg.build();

        // add dirs to header search path
        let build_dir = out_dir.join("build");
        bindings = bindings
            .clang_arg(format!("-I{build_dir}"))
            .clang_arg(format!("-I{vendor_dir}"))
            .clang_arg(format!("-I{vendor_dir}/include"));

        // link using static library
        println!("cargo::rustc-link-search=native={build_dir}");
        println!("cargo::rustc-link-lib=static=scallop");
    };

    // rerun if any vendored bash file changes
    println!("cargo::rerun-if-changed={vendor_dir}");

    #[rustfmt::skip]
    // generate bash bindings
    bindings
        .header("builtins.h")
        .allowlist_var("BUILTIN_.*")
        .allowlist_var(".*_BUILTIN")
        .allowlist_var("current_builtin")
        .allowlist_var("num_shell_builtins")
        .allowlist_var("shell_builtins")

        .header("error.h")
        .allowlist_var("SHM_BUF")

        .header("shell.h")
        .allowlist_function("bash_main")
        .allowlist_function("lib_error_handlers")
        .allowlist_function("lib_init")
        .allowlist_function("lib_reset")
        .allowlist_function("scallop_toggle_restricted")
        .allowlist_function("set_shell_name")
        .allowlist_var("shell_name")
        .allowlist_var("dist_version")
        .allowlist_var("patch_level")
        .allowlist_var("EXECUTION_FAILURE")
        .allowlist_var("EXECUTION_SUCCESS")
        .allowlist_var("EX_LONGJMP")

        .header("builtins/common.h")
        .allowlist_function("scallop_evalstring")
        .allowlist_function("scallop_source_file")
        .allowlist_function("register_builtins")
        .allowlist_function("builtin_address_internal")
        .allowlist_function("get_minus_o_opts")
        .allowlist_function("get_shopt_options")
        .allowlist_var("SEVAL_.*")
        .allowlist_var("this_shell_builtin")

        .header("command.h")
        .allowlist_function("copy_command")
        .allowlist_type("word_desc")
        .allowlist_type("word_list")
        .allowlist_var("global_command")
        .allowlist_var("CMD_.*")

        .header("execute_cmd.h")
        .allowlist_function("executing_line_number")
        .allowlist_function("scallop_execute_command")
        .allowlist_function("scallop_execute_shell_function")
        .allowlist_var("this_command_name")
        .allowlist_var("subshell_level")

        .header("variables.h")
        .allowlist_function("all_shell_variables")
        .allowlist_function("all_shell_functions")
        .allowlist_function("all_visible_variables")
        .allowlist_function("all_visible_functions")
        .allowlist_function("all_exported_variables")
        .allowlist_function("local_exported_variables")
        .allowlist_function("all_local_variables")
        .allowlist_function("all_array_variables")
        .allowlist_function("all_variables_matching_prefix")
        .allowlist_function("get_variable_value")
        .allowlist_function("get_string_value")
        .allowlist_function("bind_variable")
        .allowlist_function("bind_global_variable")
        .allowlist_function("unbind_variable")
        .allowlist_function("check_unbind_variable")
        .allowlist_function("find_function")
        .allowlist_function("find_variable")
        .allowlist_function("make_new_array_variable")
        .allowlist_function("push_context")
        .allowlist_function("pop_context")
        .allowlist_var("temporary_env")
        .allowlist_var("att_.*") // variable attributes

        .header("jobs.h")
        .allowlist_function("set_sigchld_handler")
        .allowlist_var("shell_pgrp")

        .header("externs.h")
        .allowlist_function("parse_command")
        .allowlist_function("strvec_to_word_list")

        .header("input.h")
        .allowlist_function("with_input_from_string")
        .allowlist_function("push_stream")
        .allowlist_function("pop_stream")

        .header("dispose_cmd.h")
        .allowlist_function("dispose_command")
        .allowlist_function("dispose_words")

        .header("subst.h")
        .allowlist_function("expand_string_to_string")
        .allowlist_function("expand_words_no_vars")
        .allowlist_function("list_string")
        .allowlist_var("ifs_value")
        .allowlist_var("ASS_.*")

        .header("pathexp.h")
        .allowlist_function("shell_glob_filename")

        .header("array.h")
        .allowlist_function("array_insert")
        .allowlist_function("array_reference")
        .allowlist_function("array_remove")
        .allowlist_function("array_dispose_element")
        .allowlist_type("ARRAY")

        // HACK: The last header is flagged as nonexistent if it doesn't use a path prefix
        // even when it exists in an explicitly defined include directory.
        .header(format!("{vendor_dir}/flags.h"))
        .allowlist_var("restricted")
        .allowlist_var("restricted_shell")

        // mangle type names to expected values
        .parse_callbacks(Box::new(BashCallback))
        .generate()
        .expect("unable to generate bindings")
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed writing bindings");
}
