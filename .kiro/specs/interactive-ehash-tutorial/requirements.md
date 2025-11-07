# Requirements Document

## Introduction

This feature implements an Interactive eHash Tutorial using Ratatui that provides a comprehensive, step-by-step educational experience for understanding and demonstrating the complete eHash (ecash hashrate) system. The tutorial builds on the existing ehashimint infrastructure to create a guided narrative that takes users from basic concepts through advanced eHash operations, showing real-time mining, minting, and wallet interactions with live data visualization.

## Glossary

- **eHash**: Cashu ecash tokens representing hashrate, earned by miners for submitting valid shares
- **Tutorial_System**: The interactive Ratatui-based educational application
- **Chapter**: A major section of the tutorial covering specific eHash concepts and operations
- **Step**: A granular action within a chapter that users complete to progress
- **Live_Dashboard**: Real-time display of mining statistics, eHash minting, and wallet balances
- **Narrative_Engine**: Component that provides contextual explanations and educational content
- **Progress_Tracker**: System that tracks user completion of tutorial steps and chapters

## Requirements

### Requirement 1: Interactive Tutorial Framework

**User Story:** As a developer learning about eHash, I want an interactive terminal-based tutorial that tells the story of three users (Pool Operator, Proxy Operator, Pioneer) so that I can understand the complete eHash ecosystem through hands-on experience.

#### Acceptance Criteria

1. WHEN the Tutorial_System starts THEN it SHALL display a welcome screen introducing the three-user narrative and navigation instructions using Ratatui
2. WHEN users navigate the tutorial THEN the system SHALL provide keyboard shortcuts for forward/backward navigation, pause/resume, and help
3. WHEN users complete a step THEN the Tutorial_System SHALL update the Progress_Tracker and highlight the next available action
4. WHEN users request help THEN the system SHALL display context-sensitive help overlays explaining current concepts
5. WHEN the tutorial runs THEN it SHALL maintain a persistent Live_Dashboard showing real-time eHash statistics and process status

### Requirement 2: Chapter 1 - "The Pool Operator's Journey"

**User Story:** As the Pool Operator, I want to set up and run a Pool with eHash minting capability so that miners can earn transferable eHash tokens for their hashrate contributions.

#### Acceptance Criteria

1. WHEN Chapter 1 begins THEN the Tutorial_System SHALL introduce the Pool Operator character and explain their role in the eHash ecosystem
2. WHEN Step 1.1 executes THEN the system SHALL guide the user through Pool configuration with eHash mint settings
3. WHEN Step 1.2 executes THEN the system SHALL start the Pool with eHash minting enabled and display the startup process
4. WHEN Step 1.3 executes THEN the system SHALL start the Pool's HTTP API server for external wallet access
5. WHEN Step 1.4 executes THEN the system SHALL display the Pool waiting for connections and explain what happens when miners connect
6. WHEN Chapter 1 completes THEN the user SHALL have a running Pool/Mint ready to serve miners and the tutorial SHALL transition to the Proxy Operator's story

### Requirement 3: Chapter 2 - "The Proxy Operator's Setup"

**User Story:** As the Proxy Operator, I want to create an eHash wallet, derive an hpub, configure a TProxy with my default mining pubkey, and connect to the Pool so that I can provide SV1→SV2 translation services while earning eHash tokens.

#### Acceptance Criteria

1. WHEN Chapter 2 begins THEN the Tutorial_System SHALL introduce the Proxy Operator character and explain their role as a service provider
2. WHEN Step 2.1 executes THEN the system SHALL guide the user through creating a new eHash wallet using CDK CLI or built-in wallet functionality
3. WHEN Step 2.2 executes THEN the system SHALL demonstrate deriving a public key from the wallet and encoding it as an hpub
4. WHEN Step 2.3 executes THEN the system SHALL show configuring TProxy with the derived hpub as default_locking_pubkey
5. WHEN Step 2.4 executes THEN the system SHALL start the TProxy and establish connection to the Pool from Chapter 1
6. WHEN Step 2.5 executes THEN the system SHALL display the TProxy waiting for downstream miners and explain the SV1→SV2 translation process
7. WHEN Chapter 2 completes THEN the user SHALL have a running TProxy connected to the Pool with their own eHash wallet and the tutorial SHALL transition to the Pioneer's story

### Requirement 4: Chapter 3 - "The Pioneer's Mining Adventure"

**User Story:** As the Pioneer, I want to create my own eHash wallet, derive an hpub, connect as a miner to the TProxy using my hpub in the user_identity field, wait for eHash to be minted, and then redeem my tokens so that I can experience the complete miner-to-wallet flow.

#### Acceptance Criteria

1. WHEN Chapter 3 begins THEN the Tutorial_System SHALL introduce the Pioneer character as an individual miner seeking to earn and redeem eHash tokens
2. WHEN Step 3.1 executes THEN the system SHALL guide the user through creating a second eHash wallet for the Pioneer
3. WHEN Step 3.2 executes THEN the system SHALL demonstrate deriving the Pioneer's public key and encoding it as an hpub
4. WHEN Step 3.3 executes THEN the system SHALL start a CPU miner connecting to the TProxy with the Pioneer's hpub as user_identity
5. WHEN the miner connects THEN the system SHALL show the TProxy receiving the connection and extracting the Pioneer's hpub from user_identity
6. WHEN the miner submits shares THEN the system SHALL display live mining statistics and show shares being forwarded upstream
7. WHEN the Pool receives shares THEN the system SHALL show eHash amount calculation using share hash leading zeros
8. WHEN eHash tokens are minted THEN the system SHALL display the Cashu mint quote creation with NUT-20 P2PK locking to the Pioneer's pubkey
9. WHEN sufficient tokens accumulate THEN the system SHALL guide the Pioneer through querying the mint for their quotes using NUT-20 authentication
10. WHEN the Pioneer redeems tokens THEN the system SHALL demonstrate the complete NUT-20 authentication and token redemption process
11. WHEN Chapter 3 completes THEN the user SHALL have experienced the complete mining-to-redemption flow as the Pioneer

### Requirement 5: Chapter 4 - "The Complete eHash Ecosystem"

**User Story:** As a tutorial user, I want to see all three users (Pool Operator, Proxy Operator, Pioneer) operating simultaneously in a complete eHash ecosystem so that I can understand the full system interactions.

#### Acceptance Criteria

1. WHEN Chapter 4 begins THEN the Tutorial_System SHALL display a comprehensive dashboard showing all three users' activities
2. WHEN Step 4.1 executes THEN the system SHALL show multiple miners (including the Pioneer) connecting to the TProxy simultaneously
3. WHEN Step 4.2 executes THEN the system SHALL demonstrate different miners using different hpubs for per-miner eHash distribution
4. WHEN Step 4.3 executes THEN the system SHALL show the Pool Operator monitoring mint statistics and total eHash issued
5. WHEN Step 4.4 executes THEN the system SHALL display the Proxy Operator tracking correlation data for all downstream miners
6. WHEN Step 4.5 executes THEN the system SHALL show multiple Pioneers redeeming their eHash tokens independently
7. WHEN Step 4.6 executes THEN the system SHALL demonstrate the economic flow: hashrate → shares → eHash → redemption
8. WHEN Chapter 4 completes THEN the user SHALL understand how all components work together in a production eHash system

### Requirement 7: Live Dashboard and Visualization

**User Story:** As a tutorial user, I want real-time visualization of all eHash operations so that I can see the system working and understand the data flows.

#### Acceptance Criteria

1. WHEN the Live_Dashboard displays THEN it SHALL show mining statistics (hashrate, shares/min, difficulty, uptime)
2. WHEN eHash operations occur THEN the dashboard SHALL display mint statistics (quotes created, tokens minted, total eHash issued)
3. WHEN wallet operations occur THEN the dashboard SHALL show wallet balances per pubkey and redemption history
4. WHEN processes run THEN the dashboard SHALL display process status (running/stopped, PID, uptime, resource usage)
5. WHEN the tutorial progresses THEN the dashboard SHALL highlight relevant sections based on current step
6. WHEN users navigate THEN the dashboard SHALL provide filtering options (by miner, by time range, by operation type)
7. WHEN data updates THEN the dashboard SHALL use smooth animations and color coding to show changes

### Requirement 8: Educational Narrative Engine

**User Story:** As a tutorial user, I want contextual explanations and educational content that helps me understand what's happening at each step.

#### Acceptance Criteria

1. WHEN each step begins THEN the Narrative_Engine SHALL display step objectives and expected outcomes
2. WHEN operations execute THEN the system SHALL provide real-time explanations of what's happening
3. WHEN technical concepts appear THEN the system SHALL offer expandable explanations with diagrams
4. WHEN errors occur THEN the system SHALL explain why they happened and how to resolve them
5. WHEN steps complete THEN the system SHALL summarize what was learned and preview the next step
6. WHEN users request details THEN the system SHALL provide deep-dive explanations of protocols, cryptography, and economics

### Requirement 9: Progress Tracking and Resume Capability

**User Story:** As a tutorial user, I want to save my progress and resume the tutorial later so that I can learn at my own pace.

#### Acceptance Criteria

1. WHEN users complete steps THEN the Progress_Tracker SHALL save completion status to persistent storage
2. WHEN users restart the tutorial THEN the system SHALL offer to resume from the last completed step
3. WHEN users navigate backward THEN the system SHALL allow reviewing previous steps without losing progress
4. WHEN users skip ahead THEN the system SHALL warn about prerequisites and offer to auto-complete them
5. WHEN tutorial sessions end THEN the system SHALL save all dashboard data and process states for resume

### Requirement 10: Configuration and Customization

**User Story:** As a tutorial user, I want to customize the tutorial experience and use my own configurations for advanced learning.

#### Acceptance Criteria

1. WHEN the tutorial starts THEN users SHALL be able to choose between guided mode (automatic) and manual mode (step-by-step confirmation)
2. WHEN users have existing configs THEN the system SHALL allow importing custom Pool, TProxy, and JDC configurations
3. WHEN users want to experiment THEN the system SHALL provide a sandbox mode for free exploration
4. WHEN advanced users participate THEN the system SHALL offer expert mode with additional technical details
5. WHEN the tutorial runs THEN users SHALL be able to adjust timing, skip animations, and customize the dashboard layout
