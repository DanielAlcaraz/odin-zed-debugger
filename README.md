# Odin Debugger Extension for Zed

This extension provides debug adapter protocol (DAP) support for the Odin programming language in the Zed editor. It uses LLDB as the underlying debugger to provide debugging capabilities for Odin programs.

> It is a first approach to create a Zed extension. It works but expect bugs and improvements.

## Features

- **LLDB Support**: Alternative debugger support using LLDB DAP adapter
- **Automatic Build Detection**: Automatically detects Odin build tasks and creates debug scenarios
- **Debug Configuration**: JSON schema validation for debug configurations

## Prerequisites

### Arch Linux Installation

```bash

# Install LLDB (alternative/backup)
sudo pacman -S lldb

# Install Odin compiler if not already installed
# Visit https://odin-lang.org for installation instructions
```

### Other Linux Distributions

```bash
# Ubuntu/Debian
sudo apt install lldb

# Fedora
sudo dnf install lldb

```

## Installation

### Install as Dev Extension

1. Clone this repository:
   ```bash
   git clone https://github.com/yourusername/zed-odin-debugger
   cd zed-odin-debugger
   ```

2. Build the extension:
   ```bash
   cargo build --release --target wasm32-unknown-unknown
   cp target/wasm32-unknown-unknown/release/odin_debugger.wasm extension.wasm
   ```

3. Install in Zed:
   - Open Zed
   - Press `Cmd+Shift+P` (or `Ctrl+Shift+P` on Linux)
   - Run "Extensions: Install Dev Extension"
   - Select this extension directory

## Usage

### Quick Start

1. **Create a debug configuration** by adding a `debug.json` file to your project root:
   ```json
   [
     {
       "adapter": "odin-lldb",
       "label": "Debug Example",
       "request": "launch",
       "program": "$ZED_WORKTREE_ROOT/example/main",
       "args": [],
       "cwd": "$ZED_WORKTREE_ROOT/example",
       "stopAtEntry": false,
       "build": {
         "command": "odin",
         "args": ["build", "example", "-out:example/main", "-debug"],
         "cwd": "$ZED_WORKTREE_ROOT"
       },
       "initCommands": ["settings set target.load-script-from-symbol-file true"]
     }
   ]
   ```

2. **Set breakpoints** by clicking in the gutter next to line numbers

3. **Start debugging** by pressing `F5` or using the command palette

### Debug Configuration Options

#### Common Options

- `type`: Debug adapter type (`"odin-gdb"` or `"odin-lldb"`)
- `name`: Display name for the configuration
- `request`: `"launch"` or `"attach"`
- `program`: Path to executable (for launch requests)
- `args`: Command line arguments array
- `cwd`: Working directory
- `stopAtEntry`: Stop at main function (boolean)
- `preLaunchTask`: Task to run before debugging

#### LLDB-Specific Options

- `lldbPath`: Path to LLDB executable (default: `"lldb"`)
- `initCommands`: Array of LLDB commands to run on startup
- `preRunCommands`: Commands to run before starting the program
- `postRunCommands`: Commands to run after program exits

### Build Tasks Integration

The extension automatically detects Odin build tasks and can create debug scenarios for them. Example task in your project's tasks:

```bash
# In your shell or task runner
odin build . -debug -out:my_program
```

When you run debug commands, the extension will:
1. Execute the build task if specified in `preLaunchTask`
2. Launch the debugger with the built executable
3. Set up Odin-specific debugging options

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
