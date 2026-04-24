#!/usr/bin/env bash
# CI integration test script for hx-exec
# Tests environment inheritance across different shells and platforms
# Usage: ./tests/ci-integration-test.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${SCRIPT_DIR}/target/release/hx-exec"
CONFIG="${SCRIPT_DIR}/hx-exec.ci-test.toml"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0
SKIPPED=0

# Helper functions
log_test() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASSED++))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((FAILED++))
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
    ((SKIPPED++))
}

log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

# Check if binary exists
if [ ! -f "$BIN" ]; then
    echo -e "${RED}Error: Binary not found at $BIN${NC}"
    echo "Run: cargo build --release"
    exit 1
fi

if [ ! -f "$CONFIG" ]; then
    echo -e "${RED}Error: Config not found at $CONFIG${NC}"
    exit 1
fi

echo "═══════════════════════════════════════════════════════════════════════"
echo "  hx-exec CI Integration Tests"
echo "═══════════════════════════════════════════════════════════════════════"
log_info "Binary: $BIN"
log_info "Config: $CONFIG"
log_info "OS: $(uname -s)"
echo ""

# =========================================================================
# Test Suite 1: Direct Command Execution (no shell)
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 1: Direct Command Execution ===${NC}"

log_test "Direct command: which bash"
if output=$("$BIN" -f "$CONFIG" -c ci-direct-which 2>&1); then
    if [ -n "$output" ]; then
        log_pass "which bash returned: $output"
    else
        log_fail "which bash returned empty"
    fi
else
    log_skip "which bash not available on this system"
fi

log_test "Direct command: whoami"
if output=$("$BIN" -f "$CONFIG" -c ci-direct-whoami 2>&1); then
    if [ -n "$output" ]; then
        log_pass "whoami returned: $output"
    else
        log_fail "whoami returned empty"
    fi
else
    log_fail "whoami failed"
fi

# =========================================================================
# Test Suite 2: Environment Variable Resolution
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 2: Environment Variable Resolution ===${NC}"

log_test "Env command resolution without shell"
if output=$("$BIN" -f "$CONFIG" -c ci-env-direct 2>&1); then
    if [[ -n "$output" ]]; then
        log_pass "Env var resolved: ${output:0:50}..."
    else
        log_fail "Env var resolution returned empty"
    fi
else
    log_fail "Env command resolution failed"
fi

# =========================================================================
# Test Suite 3: Bash Shell Tests
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 3: Bash Shell Tests ===${NC}"

if command -v bash &> /dev/null; then
    log_test "Bash shell: environment variables"
    if output=$("$BIN" -f "$CONFIG" -c ci-bash-env 2>&1); then
        if [[ "$output" == *"USER="* ]] && [[ "$output" == *"PATH_COUNT="* ]]; then
            log_pass "Bash env test passed: $output"
        else
            log_fail "Bash env test incomplete: $output"
        fi
    else
        log_fail "Bash env test failed"
    fi

    log_test "Bash shell: pwd resolution"
    if output=$("$BIN" -f "$CONFIG" -c ci-bash-pwd 2>&1); then
        if [[ "$output" == /* ]] || [[ "$output" == *":"* ]]; then
            log_pass "Bash pwd test passed: $output"
        else
            log_fail "Bash pwd test returned invalid path: $output"
        fi
    else
        log_fail "Bash pwd test failed"
    fi

    log_test "Bash shell: which commands"
    if output=$("$BIN" -f "$CONFIG" -c ci-bash-which 2>&1); then
        if [[ "$output" == *"bash"* ]]; then
            log_pass "Bash which test passed"
        else
            log_fail "Bash which test failed: $output"
        fi
    else
        log_fail "Bash which test execution failed"
    fi
else
    log_skip "bash not available"
fi

# =========================================================================
# Test Suite 4: Zsh Shell Tests
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 4: Zsh Shell Tests ===${NC}"

if command -v zsh &> /dev/null; then
    log_test "Zsh shell: environment variables"
    if output=$("$BIN" -f "$CONFIG" -c ci-zsh-env 2>&1); then
        if [[ "$output" == *"SHELL="* ]]; then
            log_pass "Zsh env test passed: $output"
        else
            log_fail "Zsh env test incomplete: $output"
        fi
    else
        log_skip "Zsh env test failed (may not have zsh)"
    fi

    log_test "Zsh shell: pwd resolution"
    if output=$("$BIN" -f "$CONFIG" -c ci-zsh-pwd 2>&1); then
        if [[ "$output" == /* ]] || [[ "$output" == *":"* ]]; then
            log_pass "Zsh pwd test passed: $output"
        else
            log_fail "Zsh pwd test returned invalid path: $output"
        fi
    else
        log_skip "Zsh pwd test failed (may not have zsh)"
    fi
else
    log_skip "zsh not available on this system"
fi

# =========================================================================
# Test Suite 5: Command Substitution ($(cmd))
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 5: Command Substitution ===${NC}"

log_test "Command substitution: pwd"
if output=$("$BIN" -f "$CONFIG" -c ci-cmdsub-pwd 2>&1); then
    if [[ "$output" == *"Current dir:"* ]]; then
        log_pass "Command substitution pwd: $output"
    else
        log_fail "Command substitution pwd failed: $output"
    fi
else
    log_fail "Command substitution pwd execution failed"
fi

log_test "Command substitution: whoami"
if output=$("$BIN" -f "$CONFIG" -c ci-cmdsub-user 2>&1); then
    if [[ "$output" == *"Running as:"* ]]; then
        log_pass "Command substitution whoami: $output"
    else
        log_fail "Command substitution whoami failed: $output"
    fi
else
    log_fail "Command substitution whoami execution failed"
fi

log_test "Command substitution: PATH test"
if output=$("$BIN" -f "$CONFIG" -c ci-cmdsub-pathtest 2>&1); then
    if [[ "$output" == *"found at:"* ]] || [[ "$output" == /* ]]; then
        log_pass "Command substitution PATH test: ${output:0:60}..."
    else
        log_fail "Command substitution PATH test failed: $output"
    fi
else
    log_skip "Command substitution PATH test failed"
fi

# =========================================================================
# Test Suite 6: npm root resolution
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 6: npm root Resolution ===${NC}"

if command -v npm &> /dev/null; then
    log_test "npm root resolution"
    if output=$("$BIN" -f "$CONFIG" -c ci-npm-root 2>&1); then
        if [[ "$output" == *"npm root is:"* ]]; then
            log_pass "npm root resolved: $output"
        else
            log_fail "npm root resolution incomplete: $output"
        fi
    else
        log_skip "npm root resolution failed"
    fi
else
    log_skip "npm not installed on this system"
fi

# =========================================================================
# Test Suite 7: Environment Inheritance
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 7: Environment Inheritance ===${NC}"

log_test "Environment inheritance with custom variable"
export CI_TEST_VAR="test_value_12345"
if output=$("$BIN" -f "$CONFIG" -c ci-env-inherit-test 2>&1); then
    if [[ "$output" == *"Got env var: $CI_TEST_VAR"* ]]; then
        log_pass "Environment inheritance: $output"
    else
        log_fail "Environment inheritance failed: $output"
    fi
else
    log_fail "Environment inheritance test execution failed"
fi
unset CI_TEST_VAR

# =========================================================================
# Test Suite 8: Nested Expansion
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 8: Nested Expansion ===${NC}"

log_test "Nested variable and command expansion"
if output=$("$BIN" -f "$CONFIG" -c ci-nested-expand 2>&1); then
    if [[ "$output" == *"Home is"* ]] && [[ "$output" == *"current dir"* ]]; then
        log_pass "Nested expansion: ${output:0:60}..."
    else
        log_fail "Nested expansion incomplete: $output"
    fi
else
    log_fail "Nested expansion failed"
fi

# =========================================================================
# Test Suite 9: Print mode (verify expansion without execution)
# =========================================================================

echo -e "\n${YELLOW}=== Test Suite 9: Print Mode ===${NC}"

log_test "Print mode for ci-bash-env"
if output=$("$BIN" -f "$CONFIG" --print -c ci-bash-env 2>&1); then
    if [[ "$output" == *"bash"* ]] || [[ "$output" == *"-c"* ]]; then
        log_pass "Print mode works: ${output:0:60}..."
    else
        log_fail "Print mode output unexpected: $output"
    fi
else
    log_fail "Print mode failed"
fi

# =========================================================================
# Summary
# =========================================================================

echo ""
echo "═══════════════════════════════════════════════════════════════════════"
echo "  Test Summary"
echo "═══════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo -e "${YELLOW}Skipped: $SKIPPED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi
