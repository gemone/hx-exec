# hx-exec CI Integration Tests

This directory contains integration tests for verifying hx-exec's environment inheritance and cross-platform shell support.

## Files

- **`hx-exec.ci-test.toml`** - Test configuration with 10 test suites covering various scenarios
- **`tests/ci-integration-test.sh`** - Bash-based test script for Unix/Linux/macOS
- **`tests/ci-integration-test.ps1`** - PowerShell-based test script for Windows
- `.github/workflows/ci.yml` - GitHub Actions CI workflow (enhanced version)

## Running Tests Locally

### macOS / Linux

```bash
# Build release binary (required for tests)
cargo build --release

# Run integration tests
bash tests/ci-integration-test.sh
```

### Windows (PowerShell)

```powershell
# Build release binary
cargo build --release

# Run integration tests
.\tests\ci-integration-test.ps1
```

## Test Coverage

### Test Suites

1. **Direct Command Execution** - Tests commands without shell
   - `which bash` - Verify command in PATH
   - `whoami` - Verify basic command execution

2. **Environment Variable Resolution** - Tests env var command resolution
   - Env command without shell
   - Env command with shell
   - Tests `env.VAR = { cmd = "..." }` syntax

3. **Bash Shell Tests** - Tests Bash shell integration
   - Environment variables inheritance
   - Current directory resolution
   - Which/where commands

4. **Zsh Shell Tests** - Tests Zsh shell integration
   - Environment variables inheritance
   - Current directory resolution

5. **PowerShell (pwsh) Tests** - Windows-specific
   - PowerShell environment variables
   - PowerShell command resolution
   - Environment inheritance in pwsh

6. **cmd.exe Tests** - Windows legacy shell
   - cmd.exe environment variables
   - cmd.exe command execution

7. **Command Substitution** - Tests `$(cmd)` expansion
   - Current directory via command substitution
   - User via command substitution
   - PATH-dependent command execution

8. **npm root Resolution** - Tests npm integration
   - Direct npm root resolution
   - npm root via shell command

9. **Environment Inheritance** - Tests env var inheritance
   - Custom environment variables
   - Custom env vars with shell commands

10. **Nested Expansion** - Tests combined expansions
    - Variable + command substitution together

## Test Configuration

All tests use the `hx-exec.ci-test.toml` configuration file which contains aliases for each test scenario.

### Test Aliases

```
# Direct commands (no shell)
ci-direct-which          - Test which bash command
ci-direct-whoami         - Test whoami command

# Environment variable resolution
ci-env-direct            - Test env command resolution

# Bash shell
ci-bash-env              - Test env vars in bash
ci-bash-pwd              - Test pwd resolution in bash
ci-bash-which            - Test which in bash

# Zsh shell
ci-zsh-env               - Test env vars in zsh
ci-zsh-pwd               - Test pwd resolution in zsh

# PowerShell (Windows)
ci-pwsh-env              - Test env vars in pwsh
ci-pwsh-which            - Test command resolution in pwsh
ci-pwsh-env-test         - Test env inheritance in pwsh

# cmd.exe (Windows)
ci-cmd-env               - Test env vars in cmd
ci-cmd-which             - Test where in cmd

# Command substitution
ci-cmdsub-pwd            - Test $(pwd)
ci-cmdsub-user           - Test $(whoami)
ci-cmdsub-pathtest       - Test PATH-dependent commands

# npm integration
ci-npm-root              - Test npm root resolution

# Environment inheritance
ci-env-inherit-test      - Test custom env var (bash)
ci-env-inherit-win       - Test custom env var (pwsh)

# Nested expansion
ci-nested-expand         - Test mixed expansion
ci-nested-pwsh           - Test mixed expansion (pwsh)
```

## GitHub Actions CI

The project uses GitHub Actions for automated testing. Three jobs run on each push/PR:

### Job 1: `build-and-test` (3 matrix configs)
- Builds and runs unit tests on Linux, macOS, Windows
- Runs: `cargo build --locked` && `cargo test --locked`

### Job 2: `integration-tests` (3 matrix configs)
- Runs integration tests on Linux, macOS, Windows
- Linux/macOS: Uses `tests/ci-integration-test.sh`
- Windows: Uses PowerShell script in CI workflow

### Job 3: `shell-matrix-tests` (3 shell variants)
- Tests on ubuntu-latest with different shells
- Tests: bash, sh, zsh
- Verifies environment inheritance across shell types

## Test Results

### Expected Behavior

All tests should pass (14+ passing, 0 failures) when:

1. ✓ Parent environment variables are inherited
2. ✓ Alias-specific env vars override parent
3. ✓ Command substitution works with PATH
4. ✓ npm and other PATH-dependent commands work
5. ✓ Different shells (bash, zsh, pwsh, cmd) work correctly
6. ✓ Environment inheritance works on Windows

### Known Limitations

- Some tests are skipped if:
  - Shell not installed on system (e.g., zsh on Windows)
  - Command not found on PATH (e.g., npm not installed)
  - Platform-specific features not available

## Exit Codes

- `0` - All tests passed
- `1` - One or more tests failed

## Output Format

Tests produce color-coded output for easy reading:

```
[TEST]  Test name
[PASS]  Test passed description
[FAIL]  Test failed description
[SKIP]  Test skipped reason
```

## Troubleshooting

### Tests fail on macOS/Linux

1. Verify Rust is installed: `rustc --version`
2. Build release binary: `cargo build --release`
3. Check bash is installed: `which bash`
4. Run with debug output: `bash -x tests/ci-integration-test.sh`

### Tests fail on Windows

1. Verify PowerShell 5.0+: `$PSVersionTable.PSVersion`
2. Build release binary: `cargo build --release`
3. Run with debug: `.\tests\ci-integration-test.ps1` with error output
4. Check PowerShell execution policy: `Get-ExecutionPolicy`

### Specific test fails

Each test suite can be run individually by calling hx-exec directly:

```bash
./target/release/hx-exec -f hx-exec.ci-test.toml -c <alias-name>
```

Example:
```bash
./target/release/hx-exec -f hx-exec.ci-test.toml -c ci-bash-env
```

## Adding New Tests

1. Add new alias to `hx-exec.ci-test.toml`
2. Add test case to appropriate test suite in the script
3. Follow naming convention: `ci-<category>-<name>`
4. Run tests locally to verify
5. Commit with other test changes

## References

- Main CI workflow: `.github/workflows/ci.yml`
- Unit tests: `src/runner.rs::tests` and `src/expand.rs::tests`
- Integration test config: `hx-exec.ci-test.toml`
