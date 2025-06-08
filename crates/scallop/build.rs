use std::{env, fs};

use bindgen::callbacks::{ItemInfo, ParseCallbacks};
use camino::Utf8PathBuf;

#[derive(Debug)]
struct BashCallback;

// rename bash data structures for consistency
impl ParseCallbacks for BashCallback {
    fn item_name(&self, item_info: ItemInfo) -> Option<String> {
        match item_info.name {
            // structs
            "word_desc" => Some("WordDesc".into()),
            "WORD_DESC" => Some("WordDesc".into()),
            "word_list" => Some("WordList".into()),
            "WORD_LIST" => Some("WordList".into()),
            "SHELL_VAR" => Some("ShellVar".into()),
            "ARRAY" => Some("Array".into()),
            "command" => Some("Command".into()),
            "builtin" => Some("Builtin".into()),
            // global mutables
            "global_command" => Some("GLOBAL_COMMAND".into()),
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
    let repo_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bash_repo_dir = repo_dir.join("bash");
    let out_dir = Utf8PathBuf::from(env::var("OUT_DIR").unwrap());
    let bash_out_dir = out_dir.join("bash");
    let bash_build_dir = bash_out_dir.join("build");
    fs::create_dir_all(&bash_out_dir).unwrap();

    let mut bash = autotools::Config::new(&bash_repo_dir);
    bash.make_args(vec![format!("-j{}", num_cpus::get())])
        .out_dir(&bash_out_dir)
        .forbid("--disable-shared")
        .forbid("--enable-static")
        .disable("readline", None)
        .disable("history", None)
        .disable("bang-history", None)
        .disable("progcomp", None)
        .without("bash-malloc", None)
        .disable("mem-scramble", None)
        .disable("net-redirections", None)
        .disable("nls", None)
        // job control is required for $PIPESTATUS
        .enable("job-control", None)
        // enable restricted shell support
        .enable("restricted", None)
        // build as a static library
        .enable("library", None)
        // NOTE: Fix build issues with GCC 15. Only required for bash < 5.3.
        .cflag("-std=gnu17")
        .make_target("libbash.a")
        .build();

    // statically link with bash library
    println!("cargo::rustc-link-search=native={bash_build_dir}");
    println!("cargo::rustc-link-lib=static=bash");

    // `cargo llvm-cov` currently appears to have somewhat naive object detection and erroneously
    // includes the config.status file causing it to error out
    let config_status = bash_out_dir.join("config.status");
    if config_status.exists() {
        fs::remove_file(config_status).expect("failed removing config.status");
    }

    #[rustfmt::skip]
    // generate bash bindings
    let bindings = bindgen::Builder::default()
        // add include dirs for clang
        .clang_arg(format!("-I{bash_build_dir}"))
        .clang_arg(format!("-I{bash_repo_dir}"))
        .clang_arg(format!("-I{bash_repo_dir}/include"))
        .clang_arg(format!("-I{bash_repo_dir}/builtins"))

        .header("bash/builtins.h")
        .allowlist_var("BUILTIN_.*")
        .allowlist_var(".*_BUILTIN")
        .allowlist_var("num_shell_builtins")
        .allowlist_var("shell_builtins")

        .header("bash/error.h")
        .allowlist_var("SHM_BUF")

        .header("bash/shell.h")
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

        .header("bash/builtins/common.h")
        .allowlist_function("scallop_evalstring")
        .allowlist_function("scallop_source_file")
        .allowlist_function("register_builtins")
        .allowlist_function("builtin_address_internal")
        .allowlist_function("get_minus_o_opts")
        .allowlist_function("get_shopt_options")
        .allowlist_var("SEVAL_.*")

        .header("bash/command.h")
        .allowlist_type("word_desc")
        .allowlist_type("word_list")
        .allowlist_var("global_command")
        .allowlist_function("copy_command")
        .allowlist_var("CMD_.*")

        .header("bash/execute_cmd.h")
        .allowlist_var("subshell_level")
        .allowlist_function("executing_line_number")
        .allowlist_function("scallop_execute_command")
        .allowlist_function("scallop_execute_shell_function")

        .header("bash/variables.h")
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

        .header("bash/jobs.h")
        .allowlist_function("set_sigchld_handler")
        .allowlist_var("shell_pgrp")

        .header("bash/externs.h")
        .allowlist_function("parse_command")
        .allowlist_function("strvec_dispose")
        .allowlist_function("strvec_to_word_list")

        .header("bash/input.h")
        .allowlist_function("with_input_from_string")
        .allowlist_function("push_stream")
        .allowlist_function("pop_stream")

        .header("bash/dispose_cmd.h")
        .allowlist_function("dispose_command")
        .allowlist_function("dispose_words")

        .header("bash/subst.h")
        .allowlist_function("expand_string_to_string")
        .allowlist_function("expand_words_no_vars")
        .allowlist_function("list_string")
        .allowlist_var("ifs_value")
        .allowlist_var("ASS_.*")

        .header("bash/pathexp.h")
        .allowlist_function("shell_glob_filename")

        .header("bash/array.h")
        .allowlist_type("ARRAY")
        .allowlist_function("array_insert")
        .allowlist_function("array_reference")
        .allowlist_function("array_remove")
        .allowlist_function("array_dispose_element")

        .header("bash/flags.h")
        .allowlist_var("restricted")
        .allowlist_var("restricted_shell")

        .header("bash/xmalloc.h")
        .allowlist_function("xfree")

        // mangle type names to expected values
        .parse_callbacks(Box::new(BashCallback))
        .generate()
        .expect("unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bash-bindings.rs"))
        .expect("failed writing bindings");

    // rerun if any bash file changes
    println!("cargo::rerun-if-changed=bash");
}
