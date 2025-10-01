# Implementation Plan

- [ ] 1. Set up Iroh dependencies and feature flags
  - Add Iroh dependencies to network-helpers Cargo.toml with feature flag
  - Configure conditional compilation for Iroh-specific code
  - Add feature flag to channels-sv2 crate to enable Iroh support
  - _Requirements: 3.1, 3.2, 3.4_

- [ ] 2. Implement core Iroh stream abstractions
  - [ ] 2.1 Create IrohStream struct with dual-channel architecture
    - Implement RPC client for control messages
    - Implement BiStream for high-throughput mining data
    - Add message routing logic between channels
    - _Requirements: 4.1, 4.2_
  
  - [ ] 2.2 Implement IrohReadHalf and IrohWriteHalf components
    - Create read half that handles both RPC responses and stream data
    - Create write half that routes messages to appropriate channels
    - Implement AsyncRead and AsyncWrite traits for compatibility
    - _Requirements: 4.1, 4.2_
  
  - [ ]* 2.3 Write unit tests for Iroh stream components
    - Test message serialization/deserialization over Iroh streams
    - Test dual-channel message routing
    - Test error handling for stream failures
    - _Requirements: 4.1, 4.2_

- [ ] 3. Implement Iroh connection management
  - [ ] 3.1 Create IrohConnection struct with unified interface
    - Implement connection establishment with peer discovery
    - Add support for both client and server connection modes
    - Implement connection splitting into read/write halves
    - _Requirements: 2.1, 2.2, 4.1_
  
  - [ ] 3.2 Implement protocol handlers for Stratum V2
    - Create StratumV2RpcHandler for control messages
    - Create StratumV2StreamHandler for mining data
    - Add message type classification and routing
    - _Requirements: 1.2, 4.1_
  
  - [ ] 3.3 Add graceful shutdown support
    - Implement shutdown method with goodbye message protocol
    - Add timeout handling for peer acknowledgments
    - Implement resource cleanup and connection state tracking
    - _Requirements: 1.4, 5.1_
  
  - [ ]* 3.4 Write unit tests for connection management
    - Test connection establishment and teardown
    - Test protocol handler message routing
    - Test graceful shutdown scenarios
    - _Requirements: 2.1, 2.2_

- [ ] 4. Implement Iroh node management
  - [ ] 4.1 Create IrohNodeManager for node lifecycle
    - Implement node initialization with configuration
    - Add peer discovery and connection management
    - Implement node ID persistence across restarts
    - _Requirements: 7.3, 5.4_
  
  - [ ] 4.2 Add configuration structures for Iroh settings
    - Create IrohNodeConfig with relay and STUN server support
    - Add TransportConfig enum with TCP and Iroh variants
    - Implement configuration validation and error reporting
    - _Requirements: 7.1, 7.2, 7.4_
  
  - [ ] 4.3 Implement dual transport support
    - Add support for listening on both TCP and Iroh simultaneously
    - Implement unified connection handling for mixed transports
    - Add connection metadata tracking for different transport types
    - _Requirements: 6.1, 6.2, 6.3_
  
  - [ ]* 4.4 Write unit tests for node management
    - Test node initialization and configuration
    - Test dual transport listener setup
    - Test peer discovery and connection establishment
    - _Requirements: 7.1, 7.2, 7.3_

- [ ] 5. Integrate with existing network-helpers abstraction
  - [ ] 5.1 Extend NetworkConnection trait for Iroh support
    - Add Iroh-specific methods to unified connection interface
    - Implement shutdown behavior enum for different transports
    - Add connection metadata and status tracking
    - _Requirements: 4.1, 4.3_
  
  - [ ] 5.2 Update network-helpers lib.rs with Iroh modules
    - Add conditional exports for Iroh connection types
    - Update error types to include Iroh-specific errors
    - Add feature flag documentation and examples
    - _Requirements: 3.1, 3.2, 5.1_
  
  - [ ] 5.3 Implement transport selection and fallback logic
    - Add automatic fallback from Iroh to TCP when configured
    - Implement transport preference and selection algorithms
    - Add connection retry logic with exponential backoff
    - _Requirements: 2.4, 1.4_
  
  - [ ]* 5.4 Write integration tests for network-helpers
    - Test transport selection and fallback scenarios
    - Test unified connection interface with different transports
    - Test error handling and recovery mechanisms
    - _Requirements: 2.4, 4.1, 4.3_

- [ ] 6. Update existing roles to support Iroh transport
  - [ ] 6.1 Extend configuration parsing in roles
    - Update jd-client config to support Iroh transport options
    - Update pool config to support dual transport listeners
    - Update translator config for Iroh upstream connections
    - _Requirements: 2.1, 6.1_
  
  - [ ] 6.2 Update connection establishment code in roles
    - Modify upstream connection logic to use transport config
    - Update downstream connection handling for multiple transports
    - Add transport-specific logging and error handling
    - _Requirements: 1.1, 2.2, 5.1, 5.2_
  
  - [ ]* 6.3 Write integration tests for role updates
    - Test end-to-end Stratum V2 communication over Iroh
    - Test mixed transport scenarios (TCP clients + Iroh clients)
    - Test configuration parsing and validation
    - _Requirements: 1.2, 6.2_

- [ ] 7. Add comprehensive error handling and logging
  - [ ] 7.1 Implement Iroh-specific error types
    - Create IrohError enum with detailed error variants
    - Add error conversion from Iroh library errors
    - Implement error recovery strategies for different failure modes
    - _Requirements: 1.4, 5.1, 5.2_
  
  - [ ] 7.2 Add structured logging for Iroh connections
    - Log connection establishment with peer information
    - Log transport selection and fallback events
    - Add performance metrics logging for different transports
    - _Requirements: 5.1, 5.2, 5.3_
  
  - [ ]* 7.3 Write error handling tests
    - Test error propagation and conversion
    - Test recovery strategies for various failure scenarios
    - Test logging output for different error conditions
    - _Requirements: 1.4, 5.1, 5.2_

- [ ] 8. Create configuration examples and documentation
  - [ ] 8.1 Create example configuration files
    - Add Iroh transport examples for jd-client
    - Add dual transport examples for pool server
    - Add fallback configuration examples
    - _Requirements: 2.1, 6.1, 7.1_
  
  - [ ] 8.2 Update README files with Iroh integration guide
    - Document feature flag usage and compilation
    - Add network topology examples and use cases
    - Document configuration options and best practices
    - _Requirements: 3.1, 7.1, 7.2_
  
  - [ ]* 8.3 Write end-to-end integration tests
    - Test complete mining workflow over Iroh transport
    - Test NAT traversal scenarios with relay servers
    - Test performance comparison between TCP and Iroh
    - _Requirements: 1.1, 1.2, 1.3_

- [ ] 9. Performance optimization and production readiness
  - [ ] 9.1 Optimize message routing and serialization
    - Implement zero-copy message passing where possible
    - Optimize protocol handler performance for high-frequency messages
    - Add connection pooling and reuse strategies
    - _Requirements: 4.1, 4.2_
  
  - [ ] 9.2 Add monitoring and metrics collection
    - Implement connection health monitoring
    - Add transport-specific performance metrics
    - Create dashboards for connection status and performance
    - _Requirements: 6.3, 5.1_
  
  - [ ]* 9.3 Write performance benchmarks
    - Benchmark latency comparison between TCP and Iroh
    - Benchmark throughput under various network conditions
    - Profile memory usage and resource consumption
    - _Requirements: 4.1, 4.2_