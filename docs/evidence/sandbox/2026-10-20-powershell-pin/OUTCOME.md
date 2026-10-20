# PowerShell launcher pin

Status: slice 1 landed in the working tree. The sandbox still accepts only a
regular PowerShell 7 `pwsh.exe`. Host commands may still use Windows PowerShell
5.1. No private PowerShell binary is vendored.

## What changed

- `A3S_SANDBOX_PWSH`, when set, is the only sandbox launcher candidate. An
  empty, relative, missing, directory, in-workspace, WindowsApps, or reparse
  value fails and does not fall through to Program Files.
- The Program Files candidate must be a regular file. The missing-launcher
  error names `winget install Microsoft.PowerShell` and states that Windows
  PowerShell 5.1 cannot run inside the AppContainer.
- Host resolution still prefers PowerShell 7, then 5.1, and skips reparse
  points. The chosen executable is not replaced after a spawn failure.
- `<env>` appends the note for the already selected launcher. PowerShell 7 and
  5.1 notes are distinct. Rendering does not start a process.

## Commands

```text
cargo fmt -p a3s-sandbox -p a3s-code-core
cargo test -p a3s-sandbox --lib -- missing_program_files_pwsh override_must_be_absolute empty_override absolute_override override_inside override_directory windows_apps_component reparse_point_launcher
cargo test -p a3s-code-core --lib -- powershell_7_sandbox_note windows_powershell_51_runner_note first_usable_host_powershell windows_shell_guidance windows_host_shell_prefers execution_target_closed bound_linux_target plain_local_context governed_local_missing unknown_bound_sandbox custom_runner_without workspace_without_command_runner
```

`a3s-sandbox`: 8 passed. `a3s-code-core`: 12 passed. Both finished with exit 0.

## Explicitly not claimed

- A private PowerShell 7 layout beside the installed app. That discovery and
  the packaging lock are the next slice; no zip was downloaded.
- Live AppContainer execution on a machine that does not have PowerShell 7.
- That `winget install Microsoft.PowerShell` was run, or that 5.1 can be made
  AppContainer-safe by widening the baseline ACL.
