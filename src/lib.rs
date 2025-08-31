use serde_json::Value;
use std::fs;
use std::path::Path;
use zed_extension_api::{
    self as zed, serde_json, DebugAdapterBinary, DebugConfig, DebugRequest, DebugScenario,
    DebugTaskDefinition, Result, StartDebuggingRequestArguments,
    StartDebuggingRequestArgumentsRequest, TaskTemplate, Worktree,
};

struct OdinDebuggerExtension;

impl zed::Extension for OdinDebuggerExtension {
    fn new() -> Self {
        Self
    }

    fn get_dap_binary(
        &mut self,
        adapter_name: String,
        config: DebugTaskDefinition,
        _user_provided_debug_adapter_path: Option<String>,
        _worktree: &Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        match adapter_name.as_str() {
            "odin-gdb" => {
                // Use GDB with Machine Interface mode
                Ok(DebugAdapterBinary {
                    command: Some("gdb".to_string()),
                    arguments: vec![
                        "--interpreter=mi".to_string(),
                        "--quiet".to_string(),
                        "--nx".to_string(),
                    ],
                    envs: vec![],
                    cwd: None,
                    connection: None,
                    request_args: StartDebuggingRequestArguments {
                        configuration: config.config,
                        request: StartDebuggingRequestArgumentsRequest::Launch,
                    },
                })
            }
            "odin-lldb" => {
                // Use LLDB DAP adapter
                Ok(DebugAdapterBinary {
                    command: Some("lldb-dap".to_string()),
                    arguments: vec![],
                    envs: vec![],
                    cwd: None,
                    connection: None,
                    request_args: StartDebuggingRequestArguments {
                        configuration: config.config,
                        request: StartDebuggingRequestArgumentsRequest::Launch,
                    },
                })
            }
            _ => Err(format!("Unknown debug adapter: {}", adapter_name)),
        }
    }

    fn dap_request_kind(
        &mut self,
        _adapter_name: String,
        config: Value,
    ) -> Result<StartDebuggingRequestArgumentsRequest, String> {
        let request = config
            .get("request")
            .and_then(|r| r.as_str())
            .unwrap_or("launch");

        match request {
            "launch" => Ok(StartDebuggingRequestArgumentsRequest::Launch),
            "attach" => Ok(StartDebuggingRequestArgumentsRequest::Attach),
            _ => Err(format!("Unknown request type: {}", request)),
        }
    }

    fn dap_config_to_scenario(&mut self, config: DebugConfig) -> Result<DebugScenario, String> {
        let program_path = self.resolve_program_path(&config)?;
        let adapter_name = if config.adapter.is_empty() {
            "odin-lldb" // Default to LLDB
        } else {
            &config.adapter
        };

        let mut dap_config = serde_json::Map::new();
        dap_config.insert("type".to_string(), Value::String(adapter_name.to_string()));
        dap_config.insert(
            "name".to_string(),
            Value::String(format!("Debug {}", &config.label)),
        );
        dap_config.insert(
            "request".to_string(),
            Value::String(match config.request {
                DebugRequest::Launch(_) => "launch".to_string(),
                DebugRequest::Attach(_) => "attach".to_string(),
            }),
        );

        dap_config.insert("program".to_string(), Value::String(program_path));

        if config.stop_on_entry.unwrap_or(false) {
            dap_config.insert("stopAtEntry".to_string(), Value::Bool(true));
        }

        // Add adapter-specific setup
        match adapter_name {
            "odin-gdb" => {
                let setup_commands = vec![
                    serde_json::json!({
                        "text": "set print pretty on",
                        "description": "Enable pretty printing",
                        "ignoreFailures": true
                    }),
                    serde_json::json!({
                        "text": "set print object on",
                        "description": "Enable object printing",
                        "ignoreFailures": true
                    }),
                ];
                dap_config.insert("setupCommands".to_string(), Value::Array(setup_commands));
            }
            "odin-lldb" => {
                let init_commands = vec!["settings set target.load-script-from-symbol-file true"];
                dap_config.insert(
                    "initCommands".to_string(),
                    Value::Array(
                        init_commands
                            .into_iter()
                            .map(|cmd| Value::String(cmd.to_string()))
                            .collect(),
                    ),
                );
            }
            _ => {}
        }

        Ok(DebugScenario {
            label: config.label.clone(),
            adapter: adapter_name.to_string(),
            config: serde_json::to_string(&Value::Object(dap_config)).unwrap(),
            tcp_connection: None,
            build: None,
        })
    }

    fn dap_locator_create_scenario(
        &mut self,
        _locator_name: String,
        build_task: TaskTemplate,
        resolved_label: String,
        debug_adapter_name: String,
    ) -> Option<DebugScenario> {
        // Check if this is an Odin build task
        if !self.is_odin_build_task(&build_task) {
            return None;
        }

        // Verify the debugger exists before creating scenario
        let debugger_command = match debug_adapter_name.as_str() {
            "odin-gdb" => "gdb",
            "odin-lldb" => "lldb-dap",
            _ => return None,
        };

        if !self.check_command_exists(debugger_command) {
            return None;
        }

        // Check if the directory has Odin files
        let task_dir = build_task.cwd.as_deref().unwrap_or(".");
        if !self.has_odin_files(Path::new(task_dir)) {
            return None;
        }

        // Extract the expected output path from the build task
        let output_path = self.extract_odin_output_path(&build_task, &resolved_label)?;

        let mut dap_config = serde_json::Map::new();
        dap_config.insert(
            "type".to_string(),
            Value::String(debug_adapter_name.clone()),
        );
        dap_config.insert(
            "name".to_string(),
            Value::String(format!("Debug {}", resolved_label)),
        );
        dap_config.insert("request".to_string(), Value::String("launch".to_string()));
        dap_config.insert("program".to_string(), Value::String(output_path));

        // Set working directory to where the task runs
        if let Some(cwd) = build_task.cwd.as_ref() {
            dap_config.insert("cwd".to_string(), Value::String(cwd.clone()));
        }

        // Add adapter-specific setup
        match debug_adapter_name.as_str() {
            "odin-gdb" => {
                let setup_commands = vec![serde_json::json!({
                    "text": "set print pretty on",
                    "description": "Enable pretty printing",
                    "ignoreFailures": true
                })];
                dap_config.insert("setupCommands".to_string(), Value::Array(setup_commands));
            }
            "odin-lldb" => {
                let init_commands = vec!["settings set target.load-script-from-symbol-file true"];
                dap_config.insert(
                    "initCommands".to_string(),
                    Value::Array(
                        init_commands
                            .into_iter()
                            .map(|cmd| Value::String(cmd.to_string()))
                            .collect(),
                    ),
                );
            }
            _ => {}
        }

        Some(DebugScenario {
            label: resolved_label.clone(),
            adapter: debug_adapter_name,
            config: serde_json::to_string(&Value::Object(dap_config)).unwrap(),
            tcp_connection: None,
            build: None, // We'll handle build separately since we need BuildTaskDefinition
        })
    }

    fn run_dap_locator(
        &mut self,
        _locator_name: String,
        _build_task: TaskTemplate,
    ) -> Result<DebugRequest, String> {
        // This is called after the build task completes
        // We can check what was actually built and adjust the program path if needed

        // For now, we'll use the static path resolution since Odin build outputs
        // are generally predictable. If you need dynamic discovery after build,
        // you could scan the output directory for executables here.

        Err("Use static path resolution from dap_locator_create_scenario".to_string())
    }
}

impl OdinDebuggerExtension {
    fn resolve_program_path(&self, config: &DebugConfig) -> Result<String, String> {
        // Try different strategies to find the program path

        // 1. If the label looks like a path or executable name, use it
        if config.label.contains('/') || config.label.ends_with(".exe") {
            return Ok(config.label.clone());
        }

        // 2. Look for common Odin executable patterns in the current directory
        let possible_paths = vec![
            format!("./{}", &config.label),
            format!("./bin/{}", &config.label),
            format!("./build/{}", &config.label),
            format!("./{}", &config.label.replace(' ', "_")),
            format!("./bin/{}", &config.label.replace(' ', "_")),
            format!("./build/{}", &config.label.replace(' ', "_")),
            // Common Odin output names
            "./main".to_string(),
            "./app".to_string(),
            format!(
                "./{}",
                self.get_directory_name().unwrap_or("main".to_string())
            ),
        ];

        // Check if any of these paths exist
        for path in possible_paths {
            if Path::new(&path).exists() {
                return Ok(path);
            }
        }

        // 3. Fall back to using Zed variables for dynamic resolution
        // Use $ZED_WORKTREE_ROOT instead of trying to resolve paths ourselves
        Ok(format!(
            "$ZED_WORKTREE_ROOT/{}",
            &config.label.replace(' ', "_")
        ))
    }

    fn get_directory_name(&self) -> Option<String> {
        // Get the current directory name as a fallback for executable name
        std::env::current_dir()
            .ok()?
            .file_name()?
            .to_str()
            .map(|s| s.to_string())
    }

    fn is_odin_build_task(&self, task: &TaskTemplate) -> bool {
        // Check if the command mentions odin and build
        let command_lower = task.command.to_lowercase();
        let has_odin = command_lower.contains("odin");
        let has_build = command_lower.contains("build")
            || task
                .args
                .iter()
                .any(|arg| arg.to_lowercase().contains("build"));

        has_odin && has_build
    }

    fn extract_odin_output_path(
        &self,
        task: &TaskTemplate,
        resolved_label: &str,
    ) -> Option<String> {
        let args = &task.args;
        let cwd = task.cwd.as_deref().unwrap_or(".");

        // Look for -out: flag first
        for (i, arg) in args.iter().enumerate() {
            if arg.starts_with("-out:") {
                return Some(arg.strip_prefix("-out:").unwrap().to_string());
            }
            if arg == "-out" && i + 1 < args.len() {
                return Some(args[i + 1].clone());
            }
        }

        // Look for -o flag (common alternative)
        for (i, arg) in args.iter().enumerate() {
            if arg == "-o" && i + 1 < args.len() {
                return Some(args[i + 1].clone());
            }
        }

        // Check if there's a specific package being built
        let package_name = self.extract_package_name(task, resolved_label);

        // Try different common output patterns and check which one exists or is most likely
        let possible_outputs = vec![
            format!("{}/{}", cwd, package_name),
            format!("{}/bin/{}", cwd, package_name),
            format!("{}/build/{}", cwd, package_name),
            format!("{}/main", cwd),
            format!("{}/app", cwd),
        ];

        // Check if any of the possible outputs already exist (from previous builds)
        for output_path in &possible_outputs {
            if Path::new(output_path).exists() {
                return Some(output_path.clone());
            }
        }

        // If none exist, return the most likely based on Odin conventions
        // Odin typically outputs to the current directory with the package name
        Some(possible_outputs[0].clone())
    }

    fn extract_package_name(&self, task: &TaskTemplate, resolved_label: &str) -> String {
        // Try to extract package name from the task

        // Look for package directory in args
        for arg in &task.args {
            if !arg.starts_with("-") && arg != "build" && arg != "run" {
                // This might be a package path
                if let Some(package_name) = Path::new(arg).file_name() {
                    if let Some(name_str) = package_name.to_str() {
                        return name_str.to_string();
                    }
                }
            }
        }

        // Fall back to using the resolved label or directory name
        if !resolved_label.is_empty() {
            resolved_label.replace(' ', "_")
        } else {
            self.get_directory_name().unwrap_or("main".to_string())
        }
    }

    fn check_command_exists(&self, command: &str) -> bool {
        // Try multiple ways to check if the command exists

        // First, try using 'which'
        if let Ok(output) = std::process::Command::new("which").arg(command).output() {
            if output.status.success() {
                return true;
            }
        }

        // Fallback: try using 'command -v' (more portable)
        if let Ok(output) = std::process::Command::new("sh")
            .arg("-c")
            .arg(&format!("command -v {}", command))
            .output()
        {
            if output.status.success() {
                return true;
            }
        }

        // Last resort: try running the command with --version
        if let Ok(output) = std::process::Command::new(command)
            .arg("--version")
            .output()
        {
            return output.status.success();
        }

        false
    }

    fn has_odin_files(&self, dir: &std::path::Path) -> bool {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "odin" {
                        return true;
                    }
                }
            }
        }
        false
    }
}

zed::register_extension!(OdinDebuggerExtension);
