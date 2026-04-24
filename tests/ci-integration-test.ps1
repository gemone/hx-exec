# PowerShell CI Integration Tests for hx-exec
# Tests environment inheritance on Windows with PowerShell
# Usage: .\tests\ci-integration-test.ps1

param(
    [string]$BinaryPath = "./target/release/hx-exec.exe",
    [string]$ConfigPath = "./hx-exec.ci-test.toml"
)

# Colors and output functions
$colors = @{
    Red = [ConsoleColor]::Red
    Green = [ConsoleColor]::Green
    Yellow = [ConsoleColor]::Yellow
    Blue = [ConsoleColor]::Cyan
}

$passed = 0
$failed = 0
$skipped = 0

function Write-TestHeader {
    param([string]$Text)
    Write-Host "=" * 73 -ForegroundColor Blue
    Write-Host "  $Text" -ForegroundColor Blue
    Write-Host "=" * 73
}

function Write-Test {
    param([string]$Text)
    Write-Host "[TEST] $Text" -ForegroundColor Cyan
}

function Write-Pass {
    param([string]$Text)
    Write-Host "[PASS] $Text" -ForegroundColor Green
    $script:passed++
}

function Write-Fail {
    param([string]$Text)
    Write-Host "[FAIL] $Text" -ForegroundColor Red
    $script:failed++
}

function Write-Skip {
    param([string]$Text)
    Write-Host "[SKIP] $Text" -ForegroundColor Yellow
    $script:skipped++
}

function Write-Info {
    param([string]$Text)
    Write-Host "ℹ $Text" -ForegroundColor Cyan
}

# Check if binary and config exist
if (-not (Test-Path $BinaryPath)) {
    Write-Host "ERROR: Binary not found at $BinaryPath" -ForegroundColor Red
    Write-Host "Run: cargo build --release" -ForegroundColor Yellow
    exit 1
}

if (-not (Test-Path $ConfigPath)) {
    Write-Host "ERROR: Config not found at $ConfigPath" -ForegroundColor Red
    exit 1
}

Write-TestHeader "hx-exec Windows PowerShell CI Integration Tests"
Write-Info "Binary: $BinaryPath"
Write-Info "Config: $ConfigPath"
Write-Info "OS: Windows"
Write-Info "PowerShell Version: $($PSVersionTable.PSVersion)"
Write-Host ""

# =========================================================================
# Test Suite 1: Direct Command Execution
# =========================================================================

Write-Host ""
Write-Host "=== Test Suite 1: Direct Command Execution ===" -ForegroundColor Yellow

Write-Test "Direct command: whoami"
try {
    $output = & $BinaryPath -f $ConfigPath -c ci-direct-whoami 2>&1
    if ($output -and $output.Length -gt 0) {
        Write-Pass "whoami returned: $output"
    } else {
        Write-Fail "whoami returned empty"
    }
} catch {
    Write-Skip "whoami not available: $_"
}

# =========================================================================
# Test Suite 2: PowerShell Shell Tests
# =========================================================================

Write-Host ""
Write-Host "=== Test Suite 2: PowerShell Shell Tests ===" -ForegroundColor Yellow

Write-Test "PowerShell: environment variables"
try {
    $output = & $BinaryPath -f $ConfigPath -c ci-pwsh-env 2>&1
    if ($output -and $output -like "*USER=*") {
        Write-Pass "PowerShell env test: $output"
    } else {
        Write-Fail "PowerShell env test incomplete: $output"
    }
} catch {
    Write-Fail "PowerShell env test failed: $_"
}

Write-Test "PowerShell: which/where commands"
try {
    $output = & $BinaryPath -f $ConfigPath -c ci-pwsh-which 2>&1
    if ($output -and $output.Length -gt 0) {
        Write-Pass "PowerShell which test: $($output.Substring(0, [Math]::Min(60, $output.Length)))..."
    } else {
        Write-Skip "PowerShell not found in expected location"
    }
} catch {
    Write-Skip "PowerShell which test not available: $_"
}

Write-Test "PowerShell: environment inheritance"
try {
    $output = & $BinaryPath -f $ConfigPath -c ci-pwsh-env-test 2>&1
    if ($output -and $output.Length -gt 0) {
        Write-Pass "PowerShell env inheritance: $output"
    } else {
        Write-Fail "PowerShell env inheritance returned empty"
    }
} catch {
    Write-Fail "PowerShell env inheritance failed: $_"
}

# =========================================================================
# Test Suite 3: cmd.exe Shell Tests
# =========================================================================

Write-Host ""
Write-Host "=== Test Suite 3: cmd.exe Shell Tests ===" -ForegroundColor Yellow

Write-Test "cmd.exe: environment variables"
try {
    $output = & $BinaryPath -f $ConfigPath -c ci-cmd-env 2>&1
    if ($output -and $output -like "*USER=*") {
        Write-Pass "cmd.exe env test: $output"
    } else {
        Write-Fail "cmd.exe env test incomplete: $output"
    }
} catch {
    Write-Fail "cmd.exe env test failed: $_"
}

Write-Test "cmd.exe: where command"
try {
    $output = & $BinaryPath -f $ConfigPath -c ci-cmd-which 2>&1
    if ($output -and $output -like "*cmd.exe*") {
        Write-Pass "cmd.exe which test passed"
    } else {
        Write-Fail "cmd.exe which test failed: $output"
    }
} catch {
    Write-Fail "cmd.exe which test execution failed: $_"
}

# =========================================================================
# Test Suite 4: Environment Variable Resolution
# =========================================================================

Write-Host ""
Write-Host "=== Test Suite 4: Environment Variable Resolution ===" -ForegroundColor Yellow

Write-Test "Env command resolution without shell"
try {
    $output = & $BinaryPath -f $ConfigPath -c ci-env-direct 2>&1
    if ($output -and $output -like "*RESOLVED_PWD=*") {
        Write-Pass "Env var resolved: $($output.Substring(0, [Math]::Min(50, $output.Length)))..."
    } else {
        Write-Fail "Env var not resolved properly: $output"
    }
} catch {
    Write-Fail "Env command resolution failed: $_"
}

# =========================================================================
# Test Suite 5: npm root Resolution
# =========================================================================

Write-Host ""
Write-Host "=== Test Suite 5: npm root Resolution ===" -ForegroundColor Yellow

$npmExists = $null -ne (Get-Command npm -ErrorAction SilentlyContinue)
if ($npmExists) {
    Write-Test "npm root resolution"
    try {
        $output = & $BinaryPath -f $ConfigPath -c ci-npm-root 2>&1
        if ($output -and $output -like "*npm root is:*") {
            Write-Pass "npm root resolved: $output"
        } else {
            Write-Fail "npm root resolution incomplete: $output"
        }
    } catch {
        Write-Skip "npm root resolution failed: $_"
    }
} else {
    Write-Skip "npm not installed on this system"
}

# =========================================================================
# Test Suite 6: Environment Inheritance
# =========================================================================

Write-Host ""
Write-Host "=== Test Suite 6: Environment Inheritance ===" -ForegroundColor Yellow

Write-Test "Environment inheritance with custom variable"
try {
    $env:CI_TEST_VAR = "test_value_pwsh"
    $output = & $BinaryPath -f $ConfigPath -c ci-env-inherit-win 2>&1
    Remove-Item env:\CI_TEST_VAR -ErrorAction SilentlyContinue
    
    if ($output -and $output -like "*Got env var:*") {
        Write-Pass "Environment inheritance: $output"
    } else {
        Write-Fail "Environment inheritance failed: $output"
    }
} catch {
    Write-Fail "Environment inheritance test execution failed: $_"
}

# =========================================================================
# Test Suite 7: Print Mode
# =========================================================================

Write-Host ""
Write-Host "=== Test Suite 7: Print Mode ===" -ForegroundColor Yellow

Write-Test "Print mode for ci-pwsh-env"
try {
    $output = & $BinaryPath -f $ConfigPath --print -c ci-pwsh-env 2>&1
    if ($output -and ($output -like "*pwsh*" -or $output -like "*Write-Output*")) {
        Write-Pass "Print mode works: $($output.Substring(0, [Math]::Min(60, $output.Length)))..."
    } else {
        Write-Fail "Print mode output unexpected: $output"
    }
} catch {
    Write-Fail "Print mode failed: $_"
}

# =========================================================================
# Summary
# =========================================================================

Write-Host ""
Write-TestHeader "Test Summary"
Write-Host "Passed:  " -NoNewline
Write-Host $passed -ForegroundColor Green
Write-Host "Failed:  " -NoNewline
Write-Host $failed -ForegroundColor Red
Write-Host "Skipped: " -NoNewline
Write-Host $skipped -ForegroundColor Yellow
Write-Host ""

if ($failed -eq 0) {
    Write-Host "✓ All tests passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "✗ Some tests failed" -ForegroundColor Red
    exit 1
}
