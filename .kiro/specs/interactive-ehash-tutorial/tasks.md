# Implementation Tasks - Interactive eHash Tutorial

This document breaks down the Interactive eHash Tutorial implementation into minimal, focused tasks to get something running quickly. We'll start with the bare minimum and iterate.

## Phase 1: Minimal Viable Tutorial (MVP) ✅ COMPLETE

### 1.1 Create basic project structure ✅
- [x] Create `test-utils/ehash-tutorial/Cargo.toml` with minimal dependencies
- [x] Create `test-utils/ehash-tutorial/src/main.rs` with basic Ratatui app
- [x] Add basic dependencies: `ratatui`, `crossterm`, `tokio`, `anyhow`, `tui-input`, `clap`
- [x] Add to workspace `Cargo.toml` (N/A - standalone binary)
- **Goal**: Get a basic TUI app that compiles and runs ✅
- **Files**: `test-utils/ehash-tutorial/Cargo.toml`, `test-utils/ehash-tutorial/src/main.rs`

### 1.2 Implement controlled CLI command system and state machine ✅
- [x] Create `src/commands.rs` with `CommandSystem` struct and command whitelist
- [x] Add whitelisted CLI commands: `pool_sv2`, `translator_sv2`, `mining_device`, `cdk-cli`, `ps`, `tail`, `curl`
- [x] Add tutorial navigation: `help`, `next`, `back`
- [x] Implement command validation against predefined templates (no arbitrary execution)
- [x] Create `src/state.rs` with basic `TutorialState` enum (Welcome, PoolOperator, ProxyOperator, Pioneer, Complete)
- [x] Add `StateTransition` enum with command-triggered transitions
- [x] Add tab completion for whitelisted commands and valid arguments only
- [x] Add helpful error messages for invalid commands
- **Goal**: Users type real CLI commands but only from secure whitelist ✅
- **Files**: `src/commands.rs`, `src/state.rs`, update `src/main.rs`

### 1.3 Create basic UI layout with command input ✅
- [x] Create UI layout in `main.rs` with (header, content, command input, footer)
- [x] Add chapter title display using basic text (no tui-big-text yet)
- [x] Add progress indicator (current chapter/step)
- [x] Add command input area at bottom using `tui-input`
- [x] Add help text showing available commands for current context
- **Goal**: Basic UI with interactive command input ✅
- **Files**: `src/main.rs` (integrated directly)

### 1.4 Add minimal chapter content with real CLI prompts ✅
- [x] Create `src/chapters.rs` with hardcoded chapter content
- [x] Add welcome screen explaining the guided CLI session approach
- [x] Add Pool Operator chapter prompting `pool_sv2 --config pool-config-ehash.toml`
- [x] Add Proxy Operator chapter prompting `cdk-cli wallet create` and `translator_sv2 --config tproxy-config-ehash.toml`
- [x] Add Pioneer chapter prompting `cdk-cli wallet create`, `mining_device --pool-address ... --user-identity ...`
- [x] Add contextual help showing real CLI commands available in each chapter
- **Goal**: User learns production CLI commands through guided practice ✅
- **Files**: `src/chapters.rs`

### 1.5 Test controlled command flow and security ✅
- [x] Run tutorial and test whitelisted command input with tab completion
- [x] Test invalid commands show helpful error messages (no execution)
- [x] Test `help` command shows available commands for current context
- [x] Test `next` and `back` commands for navigation
- [x] Test tab completion only suggests valid options
- [x] Test command history with up/down arrows
- [x] Verify no arbitrary command execution is possible
- **Goal**: Secure, controlled CLI experience works end-to-end ✅
- **Files**: 11 passing unit tests, `TESTING.md` manual test guide

## Phase 2: Add ehashimint Integration

### 2.1 Add ehashimint dependency
- [ ] Add `ehashimint` as path dependency to Cargo.toml
- [ ] Import ehashimint's `ProcessManager` and `config` modules
- [ ] Add basic error handling for process operations
- **Goal**: Can use ehashimint functionality
- **Files**: `Cargo.toml`, update imports

### 2.2 Add process management to state
- [ ] Add `ProcessManager` field to tutorial state
- [ ] Add process status tracking (running/stopped)
- [ ] Add basic process events (started, stopped, failed)
- **Goal**: Tutorial can track process status
- **Files**: `src/state.rs`

### 2.3 Implement Pool Operator chapter with real CLI execution
- [ ] Execute real `pool_sv2 --config pool-config-ehash.toml` command when user types it
- [ ] Generate pool-config-ehash.toml with eHash mint settings (use ehashimint defaults)
- [ ] Add process status display in UI after command execution
- [ ] Add `ps aux | grep pool_sv2` command to check if Pool is running
- [ ] Add `tail -f logs/pool.log` command to view Pool logs
- [ ] Add waiting for process startup before allowing next step
- **Goal**: Real `pool_sv2` binary starts when user types the command
- **Files**: `src/commands.rs`, `src/chapters.rs`, `src/ui.rs`

### 2.4 Add basic process monitoring
- [ ] Add simple log monitoring (tail last few lines)
- [ ] Add process health checking (is PID still running)
- [ ] Display process status in UI (Running/Stopped/Failed)
- **Goal**: User can see if processes are working
- **Files**: `src/process.rs` (new), update UI

### 2.5 Test Pool chapter end-to-end
- [ ] Run tutorial and start Pool chapter
- [ ] Verify Pool process actually starts
- [ ] Check process status display works
- [ ] Test error handling if Pool fails to start
- **Goal**: Pool chapter works with real process
- **Files**: Manual testing and fixes

## Phase 3: Add Wallet Integration

### 3.1 Add basic wallet operations
- [ ] Add simple wallet creation using CDK CLI or basic key generation
- [ ] Add hpub encoding/decoding functions
- [ ] Add wallet info display (pubkey, hpub)
- **Goal**: Can create wallets and display hpub addresses
- **Files**: `src/wallet.rs` (new)

### 3.2 Implement Proxy Operator chapter with real CDK commands
- [ ] Execute real `cdk-cli wallet create --name proxy-wallet --mint-url http://127.0.0.1:3338` when user types it
- [ ] Execute `cdk-cli wallet info proxy-wallet` to display wallet details and derive hpub
- [ ] Generate tproxy-config-ehash.toml with the wallet's hpub as default_locking_pubkey
- [ ] Execute real `translator_sv2 --config tproxy-config-ehash.toml` when user types it
- [ ] Add tab completion for wallet names and mint URLs
- [ ] Add `ps aux | grep translator_sv2` and `cdk-cli wallet balance proxy-wallet` commands
- **Goal**: User creates real CDK wallet and starts real TProxy by typing production commands
- **Files**: Update `src/commands.rs`, `src/chapters.rs`

### 3.3 Implement Pioneer chapter with real mining and wallet commands
- [ ] Execute real `cdk-cli wallet create --name pioneer-wallet --mint-url http://127.0.0.1:3338`
- [ ] Execute `cdk-cli wallet info pioneer-wallet` to get hpub for mining
- [ ] Execute real `mining_device --pool-address 127.0.0.1:34255 --user-identity hpub1...` with Pioneer's hpub
- [ ] Add `cdk-cli wallet balance pioneer-wallet` to check for eHash tokens
- [ ] Add `curl http://127.0.0.1:3338/v1/mint/quotes/pubkey/{pubkey}` to query mint directly
- [ ] Add `cdk-cli wallet redeem pioneer-wallet` to redeem tokens
- [ ] Add tab completion for wallet names, pool addresses, and hpub values
- **Goal**: Pioneer learns complete production workflow: wallet creation → mining → token redemption
- **Files**: Update `src/commands.rs`, `src/chapters.rs`

### 3.4 Test complete flow
- [ ] Run full tutorial: Pool → TProxy → Miner
- [ ] Verify all processes start correctly
- [ ] Check hpub addresses are generated and used
- [ ] Test basic error recovery
- **Goal**: Complete tutorial flow works with real processes
- **Files**: Manual testing and fixes

## Phase 4: Add Real-time Updates

### 4.1 Add event system
- [ ] Create basic event channel (tokio mpsc)
- [ ] Add process events (started, log lines, health checks)
- [ ] Add event processing loop
- **Goal**: Tutorial receives real-time updates from processes
- **Files**: `src/events.rs` (new)

### 4.2 Add live dashboard
- [ ] Add simple process status widget
- [ ] Add basic mining stats (if available from logs)
- [ ] Add live log viewer (last 10 lines)
- **Goal**: User sees live updates from running processes
- **Files**: `src/dashboard.rs` (new), update UI

### 4.3 Add eHash event detection
- [ ] Parse logs for share submissions
- [ ] Parse logs for eHash minting events
- [ ] Display eHash statistics in dashboard
- **Goal**: Tutorial shows real eHash operations
- **Files**: Update `src/events.rs` and dashboard

### 4.4 Test live updates
- [ ] Run tutorial and verify live updates work
- [ ] Check dashboard updates in real-time
- [ ] Test with actual mining and eHash minting
- **Goal**: Live tutorial experience works
- **Files**: Manual testing and fixes

## Phase 5: Polish and Enhancement

### 5.1 Add better UI components
- [ ] Add `tui-big-text` for chapter titles
- [ ] Add `tui-scrollview` for long content
- [ ] Add `color-eyre` for better error messages
- **Goal**: More polished UI experience
- **Files**: Update Cargo.toml and UI code

### 5.2 Add help system
- [ ] Add `tui-popup` for help overlays
- [ ] Add contextual help for each chapter
- [ ] Add keyboard shortcuts help
- **Goal**: Better user guidance
- **Files**: `src/help.rs` (new)

### 5.3 Add configuration options
- [ ] Add tutorial config file support
- [ ] Add command-line options (verbose, test-dir, etc.)
- [ ] Add progress saving/loading
- **Goal**: Configurable tutorial experience
- **Files**: `src/config.rs` (new)

### 5.4 Add error recovery
- [ ] Add retry mechanisms for failed processes
- [ ] Add skip options for optional steps
- [ ] Add reset functionality
- **Goal**: Robust error handling
- **Files**: Update state machine and error handling

## Phase 6: WASM Preparation (Future)

### 6.1 Add feature flags
- [ ] Add `native` and `wasm` feature flags
- [ ] Conditional compilation for process management
- [ ] Prepare for WASM-compatible alternatives
- **Goal**: Ready for WASM port
- **Files**: Cargo.toml, conditional compilation

### 6.2 Create WASM-compatible services
- [ ] Create in-memory database alternative
- [ ] Create in-process service communication
- [ ] Create simulated mining for WASM
- **Goal**: WASM version foundation
- **Files**: `src/wasm/` (new directory)

## Interactive Command Philosophy

### Guided CLI Session (Controlled Environment)
Users type **real CLI commands** but only from a predefined whitelist for security and flow control:

**Example Flow:**
```
┌─ Pool Operator Chapter ─────────────────────────────────────┐
│ Welcome, Pool Operator! Let's set up your eHash mint.       │
│                                                              │
│ First, let's start the Pool with eHash minting enabled.     │
│ Type the following command:                                  │
│                                                              │
│   pool_sv2 --config pool-config-ehash.toml                  │
│                                                              │
│ This will launch the Pool process with eHash minting        │
│ configured. The config file has already been generated      │
│ for you with the correct eHash settings.                    │
│                                                              │
│ Tutorial commands: help, next, back                         │
└──────────────────────────────────────────────────────────────┘
┌─ Command ────────────────────────────────────────────────────┐
│ $ pool_sv2 --config pool-config-ehash.toml█                 │
│   [Tab for completion]                                       │
└──────────────────────────────────────────────────────────────┘
```

### Tab Completion Examples

**Real CLI Command Completion:**
```
$ pool_[TAB]
$ pool_sv2

$ pool_sv2 --[TAB]
$ pool_sv2 --config    --help    --version

$ pool_sv2 --config [TAB]
$ pool_sv2 --config pool-config-ehash.toml
```

**CDK Wallet Commands:**
```
$ cdk-cli [TAB]
$ cdk-cli wallet    mint    info

$ cdk-cli wallet [TAB]
$ cdk-cli wallet create    balance    send    receive

$ cdk-cli wallet create --[TAB]
$ cdk-cli wallet create --name    --mint-url
```

**Context-Aware Real Commands:**
- Pool Operator: `pool_sv2 --config pool-config-ehash.toml`
- Proxy Operator: `cdk-cli wallet create --name proxy-wallet`, `translator_sv2 --config tproxy-config-ehash.toml`
- Pioneer: `cdk-cli wallet create --name pioneer-wallet`, `mining_device --pool-address 127.0.0.1:34255 --user-identity hpub1...`
- Monitoring: `cdk-cli wallet balance`, `cdk-cli mint info http://127.0.0.1:3338`

### Command History
- **Up Arrow**: Navigate to previous commands
- **Down Arrow**: Navigate to next commands
- **Persistent**: History saved across tutorial sessions

### Security and Flow Control

**Whitelisted Commands Only:**
- Users can ONLY execute commands from our predefined list
- Invalid commands show helpful error: "Command not available in tutorial. Try 'help' to see available commands."
- Tab completion only suggests valid commands and arguments
- No arbitrary command execution - prevents accidents and maintains tutorial flow

**Predefined Command Set:**
```rust
// Allowed commands with exact syntax validation
const ALLOWED_COMMANDS: &[&str] = &[
    "pool_sv2 --config pool-config-ehash.toml",
    "translator_sv2 --config tproxy-config-ehash.toml", 
    "mining_device --pool-address 127.0.0.1:34255 --user-identity {hpub}",
    "cdk-cli wallet create --name {wallet_name} --mint-url http://127.0.0.1:3338",
    "cdk-cli wallet info {wallet_name}",
    "cdk-cli wallet balance {wallet_name}",
    "ps aux | grep -E '(pool_sv2|translator_sv2|mining_device)'",
    "tail -f logs/{process}.log",
    "curl http://127.0.0.1:3338/v1/info",
    "curl http://127.0.0.1:3338/v1/mint/quotes/pubkey/{pubkey}",
    // Tutorial navigation
    "help", "next", "back"
];
```

### Educational Benefits
1. **Production Ready**: Users learn exact command syntax they'll use in real deployments
2. **Safe Learning**: Controlled environment prevents accidental system damage
3. **CLI Mastery**: Build familiarity with pool_sv2, translator_sv2, mining_device, cdk-cli
4. **Guided Discovery**: Tab completion reveals valid options within tutorial context
5. **Reference Guide**: Command history becomes a personal eHash deployment cheatsheet
6. **Confidence**: Hands-on CLI experience builds operational confidence
7. **Flow Integrity**: Maintains tutorial progression without unexpected interruptions

## Development Guidelines

### Minimal First Approach
- **Start Simple**: Get basic functionality working before adding features
- **Real Integration**: Use real ehashimint processes from the start
- **Iterate Fast**: Focus on getting something running, then improve
- **Skip Tests Initially**: Manual testing only until core functionality works

### Dependencies Strategy
- **Phase 1**: Minimal deps (ratatui, crossterm, tokio, anyhow, tui-input, clap)
- **Phase 2**: Add ehashimint integration
- **Phase 3**: Add wallet/CDK dependencies as needed
- **Phase 4**: Add event processing
- **Phase 5**: Add UI enhancement crates (tui-big-text, tui-scrollview, color-eyre)

### Error Handling
- **Phase 1**: Basic `anyhow::Result` everywhere
- **Phase 2**: Add process-specific error types
- **Phase 5**: Upgrade to `color-eyre` for better UX

### Testing Strategy
- **Phase 1-4**: Manual testing only
- **Phase 5**: Add basic unit tests for state machine
- **Phase 6**: Add integration tests for WASM compatibility

## Success Criteria

### Phase 1 Success
- ✅ Tutorial compiles and runs
- ✅ Interactive command input with tab completion works
- ✅ Can navigate between chapters using `next` and `back` commands
- ✅ `help` command shows available commands for current context
- ✅ Command history works with up/down arrows

### Phase 2 Success  
- ✅ Can start real Pool process from tutorial
- ✅ Process status is visible in UI
- ✅ Basic error handling works

### Phase 3 Success
- ✅ Can create wallets and generate hpubs
- ✅ All three processes (Pool, TProxy, Miner) start correctly
- ✅ Complete tutorial flow works end-to-end

### Phase 4 Success
- ✅ Live updates from processes appear in UI
- ✅ eHash operations are visible in real-time
- ✅ Dashboard shows meaningful statistics

This minimal approach gets us to a working tutorial quickly, then we can iterate and add features based on what works best.