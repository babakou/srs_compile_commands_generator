use std::io::Write;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Default)]
struct CompDBConf {
    common : CommonConf,
    workspace : Vec<WorkSpaceConf>
}

#[derive(Serialize, Deserialize, Default)]
struct CommonConf {
    c_compiler : String,
    cpp_compiler : String,
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
    command : Vec<String>,
    file : &'a str,
}

fn get_slashed_path_without_prefix(path : &PathBuf, prefix: &PathBuf) -> PathBuf {
    if path.starts_with(prefix) {
        path.strip_prefix(prefix).unwrap().to_str().unwrap().replace("\\", "/").into()
    } else {
        path.to_str().unwrap().replace("\\", "/").into()
    }
}

fn list_include_dirs(
    common_root : &PathBuf, 
    common_include_conf : &Option<IncludeConf>, 
    workspace_include_conf : &Option<IncludeConf>) -> Vec<PathBuf> {

    fn build_include_roots_from_include_conf(
        common_root : &PathBuf,
        common_include_conf : &Option<IncludeConf>,
        workspace_include_conf : &Option<IncludeConf>,
    ) -> Vec<PathBuf> {
        fn add_include_roots_from_include_conf(org : &mut Vec<PathBuf>, common_root : &PathBuf, include_conf : &Option<IncludeConf>) {
            if let Some(include_conf) = include_conf {            
                if let Some(include_roots) = include_conf.root_dir.as_ref() {
                    for include_root in include_roots {
                        let include_root_as_path = std::path::PathBuf::from(include_root);
                        if include_root_as_path.is_relative() {
                            org.push(common_root.join(include_root_as_path));
                        } else {
                            org.push(include_root_as_path);
                        }
                    }
                }
            }
        }

        let mut include_roots = Vec::<PathBuf>::new();
        add_include_roots_from_include_conf(&mut include_roots, common_root, common_include_conf);
        add_include_roots_from_include_conf(&mut include_roots, common_root, workspace_include_conf);

        include_roots
    }

    let include_roots = build_include_roots_from_include_conf(common_root, common_include_conf, workspace_include_conf);

    fn add_dirs_under_the_root(include_dirs : &mut Vec<PathBuf>, root_dir : &PathBuf, prefix : &PathBuf, ignore_pattern : &Vec<String>) {

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

        let read_dir = sweet_fs::fs::read_dir::ReadDir {
            dirs : true,
            recursive : true,
            files : false,
            root : true,
        };
        if let Ok(dirs) = read_dir.read(root_dir) {
            for dir in dirs {
                if dir.is_file() {
                    // If dir is not the directory, it means root_dir is the leaf directory.
                    // In this case, only push root_dir and stop the iteration.
                    add_include_dirs_if_not_ignored(include_dirs, &get_slashed_path_without_prefix(&root_dir, prefix), ignore_pattern);
                    break;
                } else {
                    add_include_dirs_if_not_ignored(include_dirs, &get_slashed_path_without_prefix(&dir, prefix), ignore_pattern);
                }
            }
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
    common_root : &PathBuf,
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
        let file_str = entry.path().to_str().unwrap();
        if target_set.is_match(file_str) && ! ignore_set.is_match(file_str) {
            target_files.push(get_slashed_path_without_prefix(&entry.into_path(), common_root));
        }
    }

    target_files
}

fn list_options(common_conf : &Option<OptionConf>, workspace_option : &Option<OptionConf>) -> Vec<String> {
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

    options
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

    let common_root = std::path::PathBuf::from(conf.common.root_dir.as_str());
    let mut compilation_db = Vec::<CompilationEntry>::new();
    for workspace in conf.workspace {
        let targets = list_target_files(&common_root, &workspace.path, &conf.common.target, &workspace.target);

        let mut options : Vec<String> = list_include_dirs(&common_root, &conf.common.include, &workspace.include).into_iter().map(|d| format!("-I{}", d.display())).collect();
        options.extend(list_options(&conf.common.option, &workspace.option));

        for target in targets {
            let mut compilation_entry = CompilationEntry::default();
            //println!("{}", target.display());
            compilation_entry.file = target.to_str().unwrap();
            if ["cc", "CC", "cpp", "CPP", "cxx", "CXX"].contains(&target.extension().unwrap_or_default().to_str().unwrap()) {
                compilation_entry.command.push(conf.common.cpp_compiler.clone());
            } else {
                compilation_entry.command.push(conf.common.c_compiler.clone());
            }
            compilation_entry.command.extend(options.clone());
            compilation_entry.directory = common_root.to_str().unwrap();

            compilation_db.push(compilation_entry);
        }
    }

    match out_file.write_all(serde_json::to_string_pretty(&compilation_db).unwrap().as_bytes()) {
        Ok(()) => println!("success to create compile_command.json"),
        Err(e) => eprintln!("failed to create compile_command.json : {}", e),
    }
}