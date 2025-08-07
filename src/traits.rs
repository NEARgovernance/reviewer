use near_sdk::{ext_contract, Promise, PromiseError};

pub type ProposalId = u32;

// External contract interfaces
#[allow(dead_code)]
#[ext_contract(ext_voting)]
pub trait VotingContract {
    #[payable]
    fn approve_proposal(&mut self, proposal_id: ProposalId, voting_start_time_sec: Option<u32>) -> Promise;
}

#[allow(dead_code)]
#[ext_contract(ext_self)]
pub trait SelfCallbacks {
    fn governance_callback(&mut self, proposal_id: ProposalId, #[callback_result] result: Result<serde_json::Value, PromiseError>);
}