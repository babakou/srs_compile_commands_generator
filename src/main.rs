use serde_json::{json, Value};
use std::fs;
use std::env;
use std::io::Write;
use glob::{glob};
use snailquote::unescape;
use std::fs::File;

#[derive(Default)]
struct Configuration {
    c_compiler_path: String,
    cpp_compiler_path: String,
    root_folder: String,
    src_pattern: Vec<String>,
    exclude_pattern: Vec<String>,
    include_folders: Vec<String>,
    compile_flags: Vec<String>
}

impl Configuration {
    pub fn display(&self) {
        println!("c_compiler_path: {}", self.c_compiler_path);
        println!("cpp_compiler_path: {}", self.cpp_compiler_path);
        println!("root_folder: {}", self.root_folder);
        println!("src_pattern: {:?}", self.src_pattern);
        println!("exclude_pattern: {:?}", self.exclude_pattern);
        println!("include_folders: {:?}", self.include_folders);
        println!("complete_flags: {:?}", self.compile_flags);
    }
}

#[derive(Default)]
struct ProjectConfiguration {
    root_configuration: Configuration,
    workspace_configurations: Vec<Configuration>,
}

impl ProjectConfiguration {
    pub fn update_root_config_from(&mut self, project_json_value: &Value) {
        self.root_configuration.c_compiler_path = unescape(project_json_value["c_compiler_path"].as_str().unwrap_or_default()).unwrap_or_default();
        self.root_configuration.cpp_compiler_path = unescape(project_json_value["cpp_compiler_path"].as_str().unwrap_or_default()).unwrap_or_default();
        self.root_configuration.root_folder = unescape(project_json_value["project_root_folder"].as_str().unwrap_or_default()).unwrap_or_default();
        if let Some(project_src_patterns) = project_json_value["project_src_pattern"].as_array() {
            for project_src_pattern in project_src_patterns {
                self.root_configuration.src_pattern.push(unescape(project_src_pattern.as_str().unwrap_or_default()).unwrap_or_default());
            }
        }
        if let Some(project_exclude_patterns) = project_json_value["project_exclude_pattern"].as_array() {
            for project_exclude_pattern in project_exclude_patterns {
                self.root_configuration.exclude_pattern.push(unescape(project_exclude_pattern.as_str().unwrap_or_default()).unwrap_or_default());
            }
        }
        if let Some(project_include_folders) = project_json_value["project_include_folders"].as_array() {
            for project_include_folder in project_include_folders {
                self.root_configuration.include_folders.push(unescape(project_include_folder.as_str().unwrap_or_default()).unwrap_or_default());
            }
        }
        if let Some(project_compile_flags) = project_json_value["project_compile_flags"].as_array() {
            for project_compile_flag in project_compile_flags {
                self.root_configuration.compile_flags.push(unescape(project_compile_flag.as_str().unwrap_or_default()).unwrap_or_default());
            }
        }

        self.root_configuration.display();
    }

    pub fn update_workspace_config_from(&mut self, project_json_value: &Value) {
        let workspaces_json_value: &Vec<Value> = project_json_value["workspaces"].as_array().unwrap();
        for workspace_json_value in workspaces_json_value {
            let mut workspace: Configuration = Configuration::default();
            workspace.c_compiler_path = unescape(workspace_json_value["c_compiler_path"].as_str().unwrap_or_default()).unwrap_or_default();
            workspace.cpp_compiler_path = unescape(workspace_json_value["cpp_compiler_path"].as_str().unwrap_or_default()).unwrap_or_default();
            workspace.root_folder = unescape(workspace_json_value["folder"].as_str().unwrap_or_default()).unwrap_or_default();
            if let Some(workspace_src_patterns) = workspace_json_value["src_pattern"].as_array() {
                for workspace_src_pattern in workspace_src_patterns {
                    workspace.src_pattern.push(unescape(workspace_src_pattern.as_str().unwrap_or_default()).unwrap_or_default());
                }
            }
            if let Some(workspace_exclude_patterns) = workspace_json_value["exclude_pattern"].as_array() {
                for workspace_exclude_pattern in workspace_exclude_patterns {
                    workspace.exclude_pattern.push(unescape(workspace_exclude_pattern.as_str().unwrap_or_default()).unwrap_or_default());
                }
            }
            if let Some(workspace_include_folders) = workspace_json_value["include_folders"].as_array() {
                let unescaped_workspace_include_folders: Vec<String> = workspace_include_folders.iter().map(|f| unescape(f.as_str().unwrap_or_default()).unwrap_or_default()).collect();
                for workspace_include_folder in unescaped_workspace_include_folders {
                    if workspace_include_folder.contains("C:/") {
                        workspace.include_folders.push(workspace_include_folder);
                    } else {
                        workspace.include_folders.push(format!("{}/{}", workspace.root_folder, workspace_include_folder));
                    }
                }
            }
            if let Some(workspace_compile_flags) = workspace_json_value["compile_flags"].as_array() {
                for workspace_compile_flag in workspace_compile_flags {
                    workspace.compile_flags.push(unescape(workspace_compile_flag.as_str().unwrap_or_default()).unwrap_or_default());
                }
            }
            workspace.display();
            self.workspace_configurations.push(workspace);
        }
    }
}

#[derive(Default)]
struct CompileCommand {
    directory: String,
    arguments: Vec<String>,
    file: String,
}


impl From<&ProjectConfiguration> for Vec<Value> {
    fn from(project_configuration: &ProjectConfiguration) -> Self {
        let mut compile_commands: Vec<Value> = Vec::new();
        for workspace in &project_configuration.workspace_configurations {
            let mut workspace_entire_include_folders = project_configuration.root_configuration.include_folders.clone();
            workspace_entire_include_folders.extend(workspace.include_folders.clone().into_iter());
            let workspace_entire_include_options: Vec<String> = workspace_entire_include_folders.iter().map(|f| format!("-include{}", f)).collect();

            let mut workspace_entire_compile_flags = project_configuration.root_configuration.compile_flags.clone();
            workspace_entire_compile_flags.extend(workspace.compile_flags.clone().into_iter());

            let mut workspace_compile_arguments: Vec<String> = Vec::new();
            workspace_compile_arguments.extend(workspace_entire_include_options.into_iter());
            workspace_compile_arguments.extend(workspace_entire_compile_flags.into_iter());
            
            let mut workspace_entire_src_patterns = project_configuration.root_configuration.src_pattern.clone();
            workspace_entire_src_patterns.extend(workspace.src_pattern.clone().into_iter());

            let mut workspace_entire_exclude_patterns = project_configuration.root_configuration.exclude_pattern.clone();
            workspace_entire_exclude_patterns.extend(workspace.exclude_pattern.clone().into_iter());

            let workspace_root_folder_absolute = format!(
                "{}/{}",
                project_configuration.root_configuration.root_folder, 
                workspace.root_folder
            );

            let mut compiled_files_candidate_list: Vec<String> = Vec::new();
            for workspace_src_pattern in workspace_entire_src_patterns {
                let workspace_src_pattern_absolute = format!(
                    "{}{}", 
                    workspace_root_folder_absolute,
                    workspace_src_pattern);
                println!("{}", workspace_src_pattern_absolute);
                for compiled_file in glob(&workspace_src_pattern_absolute).unwrap().map(|p| p.unwrap()).into_iter() {
                    compiled_files_candidate_list.push(compiled_file.display().to_string().replace("\\", "/"));
                }
            }
            let mut excluded_files_list: Vec<String> = Vec::new();
            for workspace_exclude_pattern in workspace_entire_exclude_patterns {
                let workspace_exclude_pattern_absolute = format!(
                    "{}{}",
                    workspace_root_folder_absolute,
                    workspace_exclude_pattern);
                println!("{}", workspace_exclude_pattern_absolute);
                for excluded_file in glob(&workspace_exclude_pattern_absolute).unwrap().map(|p| p.unwrap()).into_iter() {
                    excluded_files_list.push(excluded_file.display().to_string().replace("\\", "/"));
                }
            }
            
            let compiled_files_list: Vec<&String> = compiled_files_candidate_list.iter().filter(|f| !(excluded_files_list.contains(f))).collect();

            for compiled_file in compiled_files_list {
                //let mut compile_command = CompileCommand::default();
                let mut compile_command = json!({
                    "directory": "",
                    "arguments": [],
                    "file": ""
                });
                let mut arguments: Vec<String> = workspace_compile_arguments.clone();
                compile_command["directory"] = Value::from(project_configuration.root_configuration.root_folder.clone());
                compile_command["file"] = Value::from(compiled_file.clone());
                if compiled_file.contains(".cpp") || compiled_file.contains(".cxx") || compiled_file.contains(".cc") {
                    arguments.insert(0, project_configuration.root_configuration.cpp_compiler_path.clone());
                } else {
                    arguments.insert(0, project_configuration.root_configuration.c_compiler_path.clone());
                }
                compile_command["arguments"] = Value::from(arguments);
                compile_commands.push(compile_command);
            }
            //println!("compiled_files_list = {:?}", compiled_files_list);
        }
        compile_commands
    }
}

fn open_project_config_file (filename: &String) -> String {
    match fs::read_to_string(filename) {
        Ok(contents) => {
            println!("open file {}", filename);
            contents
        }
        Err(_) => {
            println!("could not open file {}", filename);
            "".to_string()
        }
    }
}

fn build_project_config_json (json_str: &String) -> Value {
    match serde_json::from_str(json_str) {
        Ok(json_value) => {
            println!("json_value was built");
            json_value
        },
        Err(_) => {
            println!("json_value was not built. Something was wrong");
            Value::Null
        }
    }
}

fn run() -> std::io::Result<()>{
    /* get filename to open */
    let args: Vec<String> = env::args().collect();
    let filename = match args.get(1) {
        Some(filename) => filename,
        None => "",
    };
    
    /* open file. If fail, exit. */
    let contents = open_project_config_file(&filename.to_string());

    /* build json object from the json string */
    let json_value: Value = build_project_config_json(&contents);

    let mut project_configuration: ProjectConfiguration = ProjectConfiguration::default();
    project_configuration.update_root_config_from(&json_value);
    project_configuration.update_workspace_config_from(&json_value);

    let compile_commands: Vec<Value> = Vec::from(&project_configuration);
    let compile_commands_json = serde_json::to_string_pretty(&compile_commands).unwrap();

    let mut compile_commands_json_file = File::create("compile_commands.json")?;

    compile_commands_json_file.write_all(compile_commands_json.as_bytes())?;

    Ok(())
}

fn main() {
    run();

    // for workspace in workspaces {
    //     println!("{}", workspace);
    // }
}