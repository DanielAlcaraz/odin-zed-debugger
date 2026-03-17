use serde_json::Value;
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
            "odin-gdb" => {
                let command = user_provided_debug_adapter_path.unwrap_or_else(|| "gdb".to_string());
                Ok(DebugAdapterBinary {
                    command: Some(command),
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
                let command = if let Some(user_path) = user_provided_debug_adapter_path {
                    user_path
                } else {
                    self.default_lldb_dap_binary()
                };

                Ok(DebugAdapterBinary {
                    command: Some(command),
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
            "odin-codelldb" => {
                let command = user_provided_debug_adapter_path.unwrap_or_else(|| "codelldb".to_string());
                Ok(DebugAdapterBinary {
                    command: Some(command),
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
        let program_path = self.resolve_program_path(&config);
        let adapter_name = if config.adapter.is_empty() {
            "odin-lldb"
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
    fn default_lldb_dap_binary(&self) -> String {
        // Query the host runtime for the actual OS
        let (platform, _) = zed::current_platform();
        match platform {
            zed::Os::Mac => "/Library/Developer/CommandLineTools/usr/bin/lldb-dap".to_string(),
            zed::Os::Linux | zed::Os::Windows => "lldb-dap".to_string(),
        }
    }

    fn resolve_program_path(&self, config: &DebugConfig) -> String {
        // Since we can't reliably test file existence from WASM, rely on Zed's variables
        if config.label.contains('/') || config.label.starts_with('$') {
            return config.label.clone();
        }

        let clean_label = config.label.replace(' ', "_");
        format!("$ZED_WORKTREE_ROOT/{}", clean_label)
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
        let cwd = task.cwd.as_deref().unwrap_or("$ZED_WORKTREE_ROOT");

        for (i, arg) in args.iter().enumerate() {
            if arg.starts_with("-out:") {
                let output = arg.strip_prefix("-out:").unwrap();
                return Some(self.make_absolute_path(output, cwd));
            }
            if (arg == "-out" || arg == "-o") && i + 1 < args.len() {
                return Some(self.make_absolute_path(&args[i + 1], cwd));
            }
        }

        let package_name = self.extract_package_name(task, resolved_label);
        Some(format!("{}/{}", cwd, package_name))
    }

    fn make_absolute_path(&self, path: &str, cwd: &str) -> String {
        if path.starts_with('/') || path.starts_with('$') {
            path.to_string()
        } else if path.starts_with("./") {
            format!("{}/{}", cwd, &path[2..])
        } else {
            format!("{}/{}", cwd, path)
        }
    }

    fn extract_package_name(&self, task: &TaskTemplate, resolved_label: &str) -> String {
        for arg in &task.args {
            if !arg.starts_with('-') && arg != "build" && arg != "run" {
                let parts: Vec<&str> = arg.split('/').collect();
                if let Some(name) = parts.last() {
                    return name.to_string();
                }
            }
        }

        if !resolved_label.is_empty() {
            resolved_label.replace(' ', "_")
        } else {
            "main".to_string()
        }
    }
}

zed::register_extension!(OdinDebuggerExtension);
