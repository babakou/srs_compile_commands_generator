use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Default)]
struct CompDBConf {
    common : CommonConf,
    workspace : Vec<WorkSpaceConf>
}

#[derive(Serialize, Deserialize, Default)]
struct CommonConf {
    c_compiler : Vec<String>,
    cpp_compiler : Vec<String>,
    root_dir : String,
    target : TargetConf,
    include : Option<IncludeConf>,
    option : Option<OptionConf>,
}

#[derive(Serialize, Deserialize, Default)]
struct WorkSpaceConf {
    path : String,
    target : Option<TargetConf>,
    include : Option<IncludeConf>,
    option : Option<OptionConf>,
}

#[derive(Serialize, Deserialize, Default)]
struct TargetConf {
    match_pattern : Option<Vec<String>>,
    ignore_pattern : Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Default)]
struct IncludeConf {
    root_dir : Option<Vec<String>>,
    ignore_pattern : Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Default)]
struct OptionConf {
    arg : Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Default)]
struct CompilationEntry<'a> {
    directory : &'a str,
    arguments : Vec<&'a str>,
    file : &'a str,
}

fn get_slashed_path_without_prefix(path : &Path, prefix: &Path) -> PathBuf {
    if path.starts_with(prefix) {
        path.strip_prefix(prefix).unwrap().to_str().unwrap().replace("\\", "/").into()
    } else {
        path.to_str().unwrap().replace("\\", "/").into()
    }
}

fn list_include_dirs(
    common_root : &Path, 
    common_include_conf : &Option<IncludeConf>, 
    workspace_root_path : &Path, 
    workspace_include_conf : &Option<IncludeConf>) -> Vec<PathBuf> {

    fn build_include_roots_from_include_conf(
        common_root : &Path,
        common_include_conf : &Option<IncludeConf>,
        workspace_root_path : &Path,
        workspace_include_conf : &Option<IncludeConf>,
    ) -> Vec<PathBuf> {
        fn add_include_roots_from_include_conf(org : &mut Vec<PathBuf>, root : &Path, include_conf : &Option<IncludeConf>) {
            if let Some(include_conf) = include_conf {            
                if let Some(include_roots) = include_conf.root_dir.as_ref() {
                    for include_root in include_roots {
                        let include_root_as_path = std::path::PathBuf::from(include_root);
                        if include_root_as_path.is_relative() {
                            org.push(root.join(include_root_as_path));
                        } else {
                            org.push(include_root_as_path);
                        }
                    }
                }
            }
        }

        let mut include_roots = Vec::<PathBuf>::new();
        add_include_roots_from_include_conf(&mut include_roots, common_root, common_include_conf);

        let workspace_root = if workspace_root_path.is_relative() {
            &common_root.join(workspace_root_path)
        } else {
            workspace_root_path
        };
        add_include_roots_from_include_conf(&mut include_roots, workspace_root, workspace_include_conf);

        include_roots
    }

    let include_roots = build_include_roots_from_include_conf(common_root, common_include_conf, workspace_root_path, workspace_include_conf);

    fn add_dirs_under_the_root(include_dirs : &mut Vec<PathBuf>, root_dir : &Path, prefix : &Path, ignore_pattern : &Vec<String>) {

        fn add_include_dirs_if_not_ignored(
            include_dirs : &mut Vec<PathBuf>,
            path : &PathBuf,
            ignore_pattern : &Vec<String>,
        ) {
            let set = regex::RegexSet::new(ignore_pattern).unwrap();

            if !set.is_match(path.to_str().unwrap()) {
                include_dirs.push(path.into());
            }
        }

        for entry in walkdir::WalkDir::new(root_dir).into_iter().filter_map(|e| e.ok()).filter(|e| e.file_type().is_dir()) {
            add_include_dirs_if_not_ignored(include_dirs, &get_slashed_path_without_prefix(&entry.into_path(), prefix), ignore_pattern);
        }
    }

    fn build_ignore_regex_patterns(common_include_conf : &Option<IncludeConf>, workspace_include_conf: &Option<IncludeConf>) -> Vec<String> {
        let mut ignore_regexp_patterns : Vec<String> = Vec::<String>::new();

        fn add_ignore_regex_pattern(patterns : &mut Vec<String>, include_conf : &Option<IncludeConf>) {
            if let Some(common_conf) = include_conf {
                if let Some(common_ignore_pattern) = &common_conf.ignore_pattern {
                    patterns.extend(common_ignore_pattern.clone());
                }
            }
        }

        add_ignore_regex_pattern(&mut ignore_regexp_patterns, common_include_conf);
        add_ignore_regex_pattern(&mut ignore_regexp_patterns, workspace_include_conf);

        ignore_regexp_patterns
    }

    let mut include_dirs = Vec::<PathBuf>::new();
    let ignore_patterns = build_ignore_regex_patterns(common_include_conf, workspace_include_conf);
    for include_root in include_roots {
        add_dirs_under_the_root(&mut include_dirs, &include_root, common_root, &ignore_patterns);
    }

    include_dirs
}

fn list_target_files(
    common_root : &Path,
    workspace_path : &String,
    common_target_conf : &TargetConf,
    workspace_target_conf : &Option<TargetConf>
    ) -> Vec<PathBuf> {
    let workspace_abs_path = common_root.join(workspace_path);
    
    let mut target_files = Vec::<PathBuf>::new();
    fn build_target_pattern(common_target_conf : &TargetConf, workspace_target_conf : &Option<TargetConf> ) -> (Vec<String>, Vec<String>) {
        let mut target_match_pattern = Vec::<String>::new();
        let mut target_ignore_pattern = Vec::<String>::new();

        target_match_pattern.extend(common_target_conf.match_pattern.clone().unwrap_or_default());
        target_ignore_pattern.extend(common_target_conf.ignore_pattern.clone().unwrap_or_default());
        if let Some(workspace_target_conf) = workspace_target_conf {
            target_match_pattern.extend(workspace_target_conf.match_pattern.clone().unwrap_or_default());
            target_ignore_pattern.extend(workspace_target_conf.ignore_pattern.clone().unwrap_or_default());
        }

        (target_match_pattern, target_ignore_pattern)
    }

    let (target_pattern, ignore_pattern) = build_target_pattern(common_target_conf, workspace_target_conf);
    let target_set = regex::RegexSet::new(target_pattern).unwrap();
    let ignore_set = regex::RegexSet::new(ignore_pattern).unwrap();

    for entry in walkdir::WalkDir::new(workspace_abs_path).into_iter().filter_map(|e| e.ok()) {
        let file_str = get_slashed_path_without_prefix(entry.path(), common_root);
        if target_set.is_match(file_str.to_str().unwrap()) && ! ignore_set.is_match(file_str.to_str().unwrap()) {
            target_files.push(file_str);
        }
    }

    target_files
}

fn list_options<'a>(common_conf : &Option<OptionConf>, workspace_option : &Option<OptionConf>) -> Vec<&'a str> {
    let mut options = Vec::<String>::new();

    fn add_options(option : &mut Vec<String>, added : &Option<OptionConf>) {
        if let Some(added) = added {
            if let Some(arg) = added.arg.as_ref() {
                option.extend(arg.clone());
            }
        }
    }

    add_options(&mut options, common_conf);
    add_options(&mut options, workspace_option);

    options.into_iter().map(|o| static_str_ops::staticize(o)).collect()
}

fn main() {
    let args : Vec<String> = std::env::args().collect();
    let input = match args.get(1) {
        Some(filename) => filename,
        None => {
            eprintln!("Input filename is reqired");
            return;
        }
    };

    let output = match args.get(2) {
        Some(dir) => format!("{}/compile_commands.json", dir),
        None => {
            eprintln!("Output directory is reqired");
            return;
        }
    };

    let mut out_file = match std::fs::File::create(output) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    
    let conf_str = std::fs::read_to_string(input).unwrap();
    let conf : CompDBConf = toml::from_str(conf_str.as_str()).unwrap();

    let common_root = std::path::PathBuf::from(conf.common.root_dir);
    let mut compilation_db = Vec::<CompilationEntry>::new();
    let workspace_arg_c : Vec<&str> = conf.common.c_compiler.iter().map(|a| a.as_str()).collect();
    let workspace_arg_cpp : Vec<&str> = conf.common.cpp_compiler.iter().map(|a| a.as_str()).collect();

    for workspace in conf.workspace {
        let targets = list_target_files(&common_root, &workspace.path, &conf.common.target, &workspace.target);

        let workspace_root = std::path::PathBuf::from(workspace.path);
        let options : Vec<String> = list_include_dirs(&common_root, &conf.common.include, &workspace_root, &workspace.include).into_iter().map(|d| format!("-I{}", d.display())).collect();
        let mut options_str : Vec<&str> = options.iter().map(|o| static_str_ops::staticize(o)).collect();
        options_str.extend(list_options(&conf.common.option, &workspace.option));

        for target in targets {
            let target_str : &'static str = static_str_ops::staticize(target.to_str().unwrap());
            let mut compilation_entry = CompilationEntry {file: target_str, ..Default::default()};
            //println!("{}", target.display());
            if ["cc", "CC", "cpp", "CPP", "cxx", "CXX"].contains(&target.extension().unwrap_or_default().to_str().unwrap()) {
                compilation_entry.arguments.extend(workspace_arg_cpp.clone());
            } else {
                compilation_entry.arguments.extend(workspace_arg_c.clone());
            }
            compilation_entry.arguments.extend(options_str.clone());
            compilation_entry.arguments.extend(["-c", target_str].into_iter());
            compilation_entry.directory = common_root.to_str().unwrap();

            compilation_db.push(compilation_entry);
        }
    }

    match out_file.write_all(serde_json::to_string_pretty(&compilation_db).unwrap().as_bytes()) {
        Ok(()) => println!("success to create compile_command.json"),
        Err(e) => eprintln!("failed to create compile_command.json : {}", e),
    }
}