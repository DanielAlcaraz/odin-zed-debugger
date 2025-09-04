use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
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
        user_provided_debug_adapter_path: Option<String>,
        _worktree: &Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        match adapter_name.as_str() {
            "odin-gdb" => Ok(DebugAdapterBinary {
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
            }),
            "odin-lldb" => {
                let lldb_dap_command = if let Some(user_path) = user_provided_debug_adapter_path {
                    user_path
                } else {
                    self.find_lldb_dap_binary()?
                };

                Ok(DebugAdapterBinary {
                    command: Some(lldb_dap_command),
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
            "odin-codelldb" => Ok(DebugAdapterBinary {
                command: Some("codelldb".to_string()),
                arguments: vec![],
                envs: vec![],
                cwd: None,
                connection: None,
                request_args: StartDebuggingRequestArguments {
                    configuration: config.config,
                    request: StartDebuggingRequestArgumentsRequest::Launch,
                },
            }),
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
            dap_config.insert("stopOnEntry".to_string(), Value::Bool(true));
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

                dap_config.insert(
                    "sourceMap".to_string(),
                    Value::Object(serde_json::Map::new()),
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
        if !self.is_odin_build_task(&build_task) {
            return None;
        }

        // Simple check: if it's LLDB on macOS, verify we have a working binary
        if debug_adapter_name == "odin-lldb" && cfg!(target_os = "macos") {
            if self.find_lldb_dap_binary().is_err() {
                return None;
            }
        }

        let task_dir = build_task.cwd.as_deref().unwrap_or(".");
        if !self.has_odin_files(Path::new(task_dir)) {
            return None;
        }

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

        if let Some(cwd) = build_task.cwd.as_ref() {
            dap_config.insert("cwd".to_string(), Value::String(cwd.clone()));
        }

        // Simple adapter setup
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
            build: None,
        })
    }

    fn run_dap_locator(
        &mut self,
        _locator_name: String,
        _build_task: TaskTemplate,
    ) -> Result<DebugRequest, String> {
        Err("Use static path resolution from dap_locator_create_scenario".to_string())
    }
}

impl OdinDebuggerExtension {
    fn find_lldb_dap_binary(&self) -> Result<String, String> {
        // Check for environment override first
        if let Ok(custom_path) = std::env::var("ODIN_LLDB_DAP_PATH") {
            if self.test_lldb_dap_binary(&custom_path) {
                return Ok(custom_path);
            }
        }

        // Platform-specific candidates - keep it simple
        let candidates = if cfg!(target_os = "macos") {
            vec![
                "/Library/Developer/CommandLineTools/usr/bin/lldb-dap",
                "/Applications/Xcode.app/Contents/Developer/usr/bin/lldb-dap",
                "/opt/homebrew/bin/lldb-vscode",
                "/opt/homebrew/opt/llvm/bin/lldb-dap",
                "/usr/local/opt/llvm/bin/lldb-dap",
                "lldb-dap",
                "lldb-vscode",
            ]
        } else {
            vec![
                "lldb-dap",
                "lldb-vscode",
                "/usr/bin/lldb-dap",
                "/usr/bin/lldb-vscode",
            ]
        };

        // Test each candidate
        for path in candidates {
            if self.test_lldb_dap_binary(path) {
                return Ok(path.to_string());
            }
        }

        Err("Could not find lldb-dap binary".to_string())
    }

    fn test_lldb_dap_binary(&self, path: &str) -> bool {
        if path.starts_with('/') && !Path::new(path).exists() {
            return false;
        }

        if let Ok(output) = Command::new(path).arg("--help").output() {
            if output.status.success() || !output.stdout.is_empty() || !output.stderr.is_empty() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}{}", stdout, stderr).to_lowercase();

                return combined.contains("lldb")
                    || combined.contains("dap")
                    || combined.contains("debug")
                    || combined.contains("vscode");
            }
        }

        false
    }

    fn resolve_program_path(&self, config: &DebugConfig) -> Result<String, String> {
        // If it looks like a path already, use it
        if config.label.contains('/') || config.label.starts_with("$") {
            return Ok(config.label.clone());
        }

        // Clean the label
        let clean_label = config.label.replace(' ', "_");

        // Try common locations
        let possible_paths = vec![
            format!("./{}", clean_label),
            format!("./bin/{}", clean_label),
            format!("./build/{}", clean_label),
            "./main".to_string(),
            "./app".to_string(),
        ];

        // Check if any exist
        for path in &possible_paths {
            if Path::new(path).exists() {
                if let Ok(abs_path) = std::fs::canonicalize(path) {
                    if let Some(path_str) = abs_path.to_str() {
                        return Ok(path_str.to_string());
                    }
                }
                return Ok(path.clone());
            }
        }

        // Fallback to Zed variable
        Ok(format!("$ZED_WORKTREE_ROOT/{}", clean_label))
    }

    fn is_odin_build_task(&self, task: &TaskTemplate) -> bool {
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

        // Look for output flags
        for (i, arg) in args.iter().enumerate() {
            if arg.starts_with("-out:") {
                let output = arg.strip_prefix("-out:").unwrap();
                return Some(self.make_absolute_path(output, cwd));
            }
            if (arg == "-out" || arg == "-o") && i + 1 < args.len() {
                return Some(self.make_absolute_path(&args[i + 1], cwd));
            }
        }

        // Default
        let package_name = self.extract_package_name(task, resolved_label);
        Some(format!("{}/{}", cwd, package_name))
    }

    fn make_absolute_path(&self, path: &str, cwd: &str) -> String {
        if path.starts_with('/') || path.starts_with("$") {
            path.to_string()
        } else if path.starts_with("./") {
            format!("{}/{}", cwd, &path[2..])
        } else {
            format!("{}/{}", cwd, path)
        }
    }

    fn extract_package_name(&self, task: &TaskTemplate, resolved_label: &str) -> String {
        // Look for package directory in args
        for arg in &task.args {
            if !arg.starts_with("-") && arg != "build" && arg != "run" {
                if let Some(package_name) = Path::new(arg).file_name() {
                    if let Some(name_str) = package_name.to_str() {
                        return name_str.to_string();
                    }
                }
            }
        }

        // Fallback
        if !resolved_label.is_empty() {
            resolved_label.replace(' ', "_")
        } else {
            "main".to_string()
        }
    }

    fn has_odin_files(&self, dir: &Path) -> bool {
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
