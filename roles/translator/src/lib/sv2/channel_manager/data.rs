use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use stratum_apps::{
    custom_mutex::Mutex,
    stratum_core::{
        bitcoin::secp256k1::PublicKey,
        channels_sv2::client::extended::ExtendedChannel,
        mining_sv2::ExtendedExtranonce,
    },
};

/// Defines the operational mode for channel management.
///
/// The channel manager can operate in two different modes that affect how
/// downstream connections are mapped to upstream SV2 channels:
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub enum ChannelMode {
    /// All downstream connections share a single extended SV2 channel.
    /// This mode uses extranonce prefix allocation to distinguish between
    /// different downstream miners while presenting them as a single entity
    /// to the upstream server. This is more efficient for pools with many
    /// miners.
    Aggregated,
    /// Each downstream connection gets its own dedicated extended SV2 channel.
    /// This mode provides complete isolation between downstream connections
    /// but may be less efficient for large numbers of miners.
    NonAggregated,
}

/// eHash-specific metadata for a channel.
///
/// Stores the locking pubkey needed when forwarding shares upstream.
#[derive(Debug, Clone)]
pub struct ChannelEHashData {
    /// The locking pubkey for eHash minting (extracted from user_identity hpub)
    pub locking_pubkey: PublicKey,
}

/// Internal data structure for the ChannelManager.
///
/// This struct maintains all the state needed for SV2 channel management,
/// including pending channel requests, active channels, and mode-specific
/// data structures like extranonce factories for aggregated mode.
#[derive(Debug, Clone)]
pub struct ChannelManagerData {
    /// Store pending channel info by request_id: (user_identity, hashrate,
    /// downstream_extranonce_len, downstream_id, locking_pubkey)
    pub pending_channels: HashMap<u32, (String, f32, usize, u32, PublicKey)>,
    /// Map of active extended channels by channel ID
    pub extended_channels: HashMap<u32, Arc<RwLock<ExtendedChannel<'static>>>>,
    /// The upstream extended channel used in aggregated mode
    pub upstream_extended_channel: Option<Arc<RwLock<ExtendedChannel<'static>>>>,
    /// Extranonce prefix factory for allocating unique prefixes in aggregated mode
    pub extranonce_prefix_factory: Option<Arc<Mutex<ExtendedExtranonce>>>,
    /// Current operational mode
    pub mode: ChannelMode,
    /// Share sequence number counter for tracking valid shares forwarded upstream.
    /// In aggregated mode: single counter for all shares going to the upstream channel.
    /// In non-aggregated mode: one counter per downstream channel.
    pub share_sequence_counters: HashMap<u32, u32>,
    /// Per-channel extranonce factories for non-aggregated mode when extranonce adjustment is
    /// needed
    pub extranonce_factories: Option<HashMap<u32, Arc<Mutex<ExtendedExtranonce>>>>,
    /// Maps channel_id -> ChannelEHashData for storing eHash-specific channel metadata.
    /// This persists for the lifetime of the channel and is used to get the locking_pubkey
    /// when forwarding SubmitSharesExtended upstream.
    pub channel_ehash_data: HashMap<u32, ChannelEHashData>,
}

impl ChannelManagerData {
    /// Creates a new ChannelManagerData instance.
    ///
    /// # Arguments
    /// * `mode` - The operational mode (Aggregated or NonAggregated)
    ///
    /// # Returns
    /// A new ChannelManagerData instance with empty state
    pub fn new(mode: ChannelMode) -> Self {
        Self {
            pending_channels: HashMap::new(),
            extended_channels: HashMap::new(),
            upstream_extended_channel: None,
            extranonce_prefix_factory: None,
            mode,
            share_sequence_counters: HashMap::new(),
            extranonce_factories: None,
            channel_ehash_data: HashMap::new(),
        }
    }

    /// Resets all channel state for upstream reconnection.
    ///
    /// This method clears all existing channel state that becomes invalid
    /// when the upstream connection is lost and reestablished. It preserves
    /// the operational mode but clears:
    /// - All pending channel requests
    /// - All active extended channels
    /// - The upstream extended channel
    /// - The extranonce prefix factory
    ///
    /// This ensures that new channels will be properly opened with the
    /// newly connected upstream server.
    pub fn reset_for_upstream_reconnection(&mut self) {
        self.pending_channels.clear();
        self.extended_channels.clear();
        self.upstream_extended_channel = None;
        self.extranonce_prefix_factory = None;
        self.share_sequence_counters.clear();
        self.extranonce_factories = None;
        self.channel_ehash_data.clear();
        // Note: we intentionally preserve `mode` as it's a configuration setting
    }

    /// Gets the next sequence number for a valid share and increments the counter.
    ///
    /// The counter_key determines which counter to use:
    /// - In aggregated mode: use upstream channel ID (single counter for all shares)
    /// - In non-aggregated mode: use downstream channel ID (one counter per channel)
    pub fn next_share_sequence_number(&mut self, counter_key: u32) -> u32 {
        let counter = self.share_sequence_counters.entry(counter_key).or_insert(1);
        let current = *counter;
        *counter += 1;
        current
    }
}
