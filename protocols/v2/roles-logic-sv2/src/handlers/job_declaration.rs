//! # Job Declaration Handlers
//!
//! This module defines traits and functions for handling job declaration messages within the
//! Stratum V2 protocol. The job declaration process is integral to managing mining tasks and
//! transactions between server and client components.
//!
//! ## Message Handling
//!
//! The handlers are responsible for the following tasks:
//! - Parsing and deserializing job declaration messages into appropriate types.
//! - Dispatching the deserialized messages to specific handler functions based on their type, such
//!   as handling job token allocation, job declaration success or error responses, and transaction
//!   data management.
//!
//! ## Return Type
//!
//! The functions return a `Result<SendTo, Error>`. The `SendTo` type determines the next action for
//! the message: whether the message should be relayed, responded to, or ignored. If an error occurs
//! during processing, the `Error` type is returned.
//!
//! ## Structure
//!
//! This module contains:
//! - Traits for processing job declaration messages, covering both server-side and client-side
//!   handling.
//! - Functions designed to parse, deserialize, and process messages related to job declarations,
//!   with robust error handling.
//! - Error handling mechanisms to address unexpected messages and ensure safe processing,
//!   particularly in the context of shared state.

use crate::{parsers::JobDeclaration, utils::Mutex};
use std::sync::Arc;

/// see [`SendTo_`]
pub type SendTo = SendTo_<JobDeclaration<'static>, ()>;
use super::SendTo_;
use crate::errors::Error;
use const_sv2::{
    MESSAGE_TYPE_ALLOCATE_MINING_JOB_TOKEN, MESSAGE_TYPE_ALLOCATE_MINING_JOB_TOKEN_SUCCESS,
    MESSAGE_TYPE_DECLARE_MINING_JOB, MESSAGE_TYPE_DECLARE_MINING_JOB_ERROR,
    MESSAGE_TYPE_DECLARE_MINING_JOB_SUCCESS, MESSAGE_TYPE_IDENTIFY_TRANSACTIONS,
    MESSAGE_TYPE_IDENTIFY_TRANSACTIONS_SUCCESS, MESSAGE_TYPE_PROVIDE_MISSING_TRANSACTIONS,
    MESSAGE_TYPE_PROVIDE_MISSING_TRANSACTIONS_SUCCESS, MESSAGE_TYPE_SUBMIT_SOLUTION_JD,
};
use core::convert::TryInto;
use job_declaration_sv2::*;
use tracing::{debug, error, info, trace};

/// A trait for parsing and handling SV2 job declaration messages sent by a server.
///
/// This trait is designed to be implemented by downstream components that need to handle
/// various job declaration messages from an upstream SV2 server, such as job tokens allocation,
/// declaration success, and error messages.
pub trait ParseJobDeclarationMessagesFromUpstream
where
    Self: Sized,
{
    /// Routes an incoming job declaration message to the appropriate handler function.
    fn handle_message_job_declaration(
        self_: Arc<Mutex<Self>>,
        message_type: u8,
        payload: &mut [u8],
    ) -> Result<SendTo, Error> {
        Self::handle_message_job_declaration_deserialized(self_, (message_type, payload).try_into())
    }

    /// Routes a deserialized job declaration message to the appropriate handler function.
    fn handle_message_job_declaration_deserialized(
        self_: Arc<Mutex<Self>>,
        message: Result<JobDeclaration<'_>, Error>,
    ) -> Result<SendTo, Error> {
        match message {
            Ok(JobDeclaration::AllocateMiningJobTokenSuccess(message)) => {
                debug!(
                    "Received AllocateMiningJobTokenSuccess with id: {}",
                    message.request_id
                );
                trace!("AllocateMiningJobTokenSuccess: {:?}", message.request_id);
                self_
                    .safe_lock(|x| x.handle_allocate_mining_job_token_success(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::DeclareMiningJobSuccess(message)) => {
                info!(
                    "Received DeclareMiningJobSuccess with id {}",
                    message.request_id
                );
                debug!("DeclareMiningJobSuccess: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_declare_mining_job_success(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::DeclareMiningJobError(message)) => {
                error!(
                    "Received DeclareMiningJobError, error code: {}",
                    std::str::from_utf8(message.error_code.as_ref())
                        .unwrap_or("unknown error code")
                );
                debug!("DeclareMiningJobSuccess: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_declare_mining_job_error(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::IdentifyTransactions(message)) => {
                info!(
                    "Received IdentifyTransactions with id: {}",
                    message.request_id
                );
                debug!("IdentifyTransactions: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_identify_transactions(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::ProvideMissingTransactions(message)) => {
                info!(
                    "Received ProvideMissingTransactions with id: {}",
                    message.request_id
                );
                debug!("ProvideMissingTransactions: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_provide_missing_transactions(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::AllocateMiningJobToken(_)) => Err(Error::UnexpectedMessage(
                MESSAGE_TYPE_ALLOCATE_MINING_JOB_TOKEN,
            )),
            Ok(JobDeclaration::DeclareMiningJob(_)) => {
                Err(Error::UnexpectedMessage(MESSAGE_TYPE_DECLARE_MINING_JOB))
            }
            Ok(JobDeclaration::ProvideMissingTransactionsSuccess(_)) => Err(
                Error::UnexpectedMessage(MESSAGE_TYPE_PROVIDE_MISSING_TRANSACTIONS_SUCCESS),
            ),
            Ok(JobDeclaration::IdentifyTransactionsSuccess(_)) => Err(Error::UnexpectedMessage(
                MESSAGE_TYPE_IDENTIFY_TRANSACTIONS_SUCCESS,
            )),
            Ok(JobDeclaration::SubmitSolution(_)) => {
                Err(Error::UnexpectedMessage(MESSAGE_TYPE_SUBMIT_SOLUTION_JD))
            }
            Err(e) => Err(e),
        }
    }

    /// Handles an `AllocateMiningJobTokenSuccess` message.
    ///
    /// This method processes a message indicating a successful job token allocation.
    fn handle_allocate_mining_job_token_success(
        &mut self,
        message: AllocateMiningJobTokenSuccess,
    ) -> Result<SendTo, Error>;

    /// Handles a `DeclareMiningJobSuccess` message.
    ///
    /// This method processes a message indicating a successful mining job declaration.
    fn handle_declare_mining_job_success(
        &mut self,
        message: DeclareMiningJobSuccess,
    ) -> Result<SendTo, Error>;

    /// Handles a `DeclareMiningJobError` message.
    ///
    /// This method processes a message indicating an error in the mining job declaration process.
    fn handle_declare_mining_job_error(
        &mut self,
        message: DeclareMiningJobError,
    ) -> Result<SendTo, Error>;

    /// Handles an `IdentifyTransactions` message.
    ///
    /// This method processes a message that provides transaction identification data.
    fn handle_identify_transactions(
        &mut self,
        message: IdentifyTransactions,
    ) -> Result<SendTo, Error>;

    /// Handles a `ProvideMissingTransactions` message.
    ///
    /// This method processes a message that supplies missing transaction data.
    fn handle_provide_missing_transactions(
        &mut self,
        message: ProvideMissingTransactions,
    ) -> Result<SendTo, Error>;
}

/// A trait responsible for handling job declaration messages sent by clients to upstream nodes.
/// The methods process messages like job declarations, solutions, and transaction success
/// indicators, ensuring proper routing and handling.
pub trait ParseJobDeclarationMessagesFromDownstream
where
    Self: Sized,
{
    /// Routes an incoming job declaration message to the appropriate handler function.
    fn handle_message_job_declaration(
        self_: Arc<Mutex<Self>>,
        message_type: u8,
        payload: &mut [u8],
    ) -> Result<SendTo, Error> {
        Self::handle_message_job_declaration_deserialized(self_, (message_type, payload).try_into())
    }

    /// Routes a deserialized job declaration message to the appropriate handler function.
    fn handle_message_job_declaration_deserialized(
        self_: Arc<Mutex<Self>>,
        message: Result<JobDeclaration<'_>, Error>,
    ) -> Result<SendTo, Error> {
        match message {
            Ok(JobDeclaration::AllocateMiningJobToken(message)) => {
                debug!(
                    "Received AllocateMiningJobToken with id: {}",
                    message.request_id
                );
                trace!("AllocateMiningJobToken: {:?}", message.request_id);
                self_
                    .safe_lock(|x| x.handle_allocate_mining_job_token(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::DeclareMiningJob(message)) => {
                info!("Received DeclareMiningJob with id: {}", message.request_id);
                debug!("DeclareMiningJob: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_declare_mining_job(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::IdentifyTransactionsSuccess(message)) => {
                info!(
                    "Received IdentifyTransactionsSuccess with id: {}",
                    message.request_id
                );
                debug!("IdentifyTransactionsSuccess: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_identify_transactions_success(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::ProvideMissingTransactionsSuccess(message)) => {
                info!(
                    "Received ProvideMissingTransactionsSuccess with id: {}",
                    message.request_id
                );
                debug!("ProvideMissingTransactionsSuccess: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_provide_missing_transactions_success(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::SubmitSolution(message)) => {
                info!("Received SubmitSolution");
                debug!("SubmitSolution: {:?}", message);
                self_
                    .safe_lock(|x| x.handle_submit_solution(message))
                    .map_err(|e| crate::Error::PoisonLock(e.to_string()))?
            }
            Ok(JobDeclaration::AllocateMiningJobTokenSuccess(_)) => Err(Error::UnexpectedMessage(
                MESSAGE_TYPE_ALLOCATE_MINING_JOB_TOKEN_SUCCESS,
            )),
            Ok(JobDeclaration::DeclareMiningJobSuccess(_)) => Err(Error::UnexpectedMessage(
                MESSAGE_TYPE_DECLARE_MINING_JOB_SUCCESS,
            )),
            Ok(JobDeclaration::DeclareMiningJobError(_)) => Err(Error::UnexpectedMessage(
                MESSAGE_TYPE_DECLARE_MINING_JOB_ERROR,
            )),
            Ok(JobDeclaration::ProvideMissingTransactions(_)) => Err(Error::UnexpectedMessage(
                MESSAGE_TYPE_PROVIDE_MISSING_TRANSACTIONS,
            )),
            Ok(JobDeclaration::IdentifyTransactions(_)) => {
                Err(Error::UnexpectedMessage(MESSAGE_TYPE_IDENTIFY_TRANSACTIONS))
            }
            Err(e) => Err(e),
        }
    }

    /// Handles an `AllocateMiningJobToken` message.
    fn handle_allocate_mining_job_token(
        &mut self,
        message: AllocateMiningJobToken,
    ) -> Result<SendTo, Error>;

    /// Handles a `DeclareMiningJob` message.
    ///
    /// This method processes a message that declares a new mining job.
    fn handle_declare_mining_job(&mut self, message: DeclareMiningJob) -> Result<SendTo, Error>;

    /// Handles an `IdentifyTransactionsSuccess` message.
    ///
    /// This method processes a message that confirms the identification of transactions.
    fn handle_identify_transactions_success(
        &mut self,
        message: IdentifyTransactionsSuccess,
    ) -> Result<SendTo, Error>;

    /// Handles a `ProvideMissingTransactionsSuccess` message.
    ///
    /// This method processes a message that confirms the receipt of missing transactions.
    fn handle_provide_missing_transactions_success(
        &mut self,
        message: ProvideMissingTransactionsSuccess,
    ) -> Result<SendTo, Error>;

    /// Handles a `SubmitSolution` message.
    ///
    /// This method processes a message that submits a solution for the mining job.
    fn handle_submit_solution(&mut self, message: SubmitSolutionJd) -> Result<SendTo, Error>;
}
