mod common;
mod error;
mod extensions;
#[cfg(feature = "job_declaration")]
mod job_declaration;
#[cfg(feature = "mining")]
mod mining;
#[cfg(feature = "template_distribution")]
mod template_distribution;

pub use error::HandlerErrorType;

pub use common::{
    HandleCommonMessagesFromClientAsync, HandleCommonMessagesFromClientSync,
    HandleCommonMessagesFromServerAsync, HandleCommonMessagesFromServerSync,
};

#[cfg(feature = "mining")]
pub use mining::{
    HandleMiningMessagesFromClientAsync, HandleMiningMessagesFromClientSync,
    HandleMiningMessagesFromServerAsync, HandleMiningMessagesFromServerSync, SupportedChannelTypes,
};

#[cfg(feature = "template_distribution")]
pub use template_distribution::{
    HandleTemplateDistributionMessagesFromClientAsync,
    HandleTemplateDistributionMessagesFromClientSync,
    HandleTemplateDistributionMessagesFromServerAsync,
    HandleTemplateDistributionMessagesFromServerSync,
};

#[cfg(feature = "job_declaration")]
pub use job_declaration::{
    HandleJobDeclarationMessagesFromClientAsync, HandleJobDeclarationMessagesFromClientSync,
    HandleJobDeclarationMessagesFromServerAsync, HandleJobDeclarationMessagesFromServerSync,
};

pub use extensions::{
    HandleExtensionsFromClientAsync, HandleExtensionsFromClientSync,
    HandleExtensionsFromServerAsync, HandleExtensionsFromServerSync,
};
