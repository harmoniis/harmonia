/// Types, structs, and enums for the payment-auth crate.

pub(crate) const COMPONENT: &str = "payment-auth";
pub(crate) const DEFAULT_ALLOWED_RAILS: &[&str] = &["webcash", "voucher", "bitcoin"];

#[derive(Debug, Clone, Default)]
pub struct InboundPaymentMetadata {
    pub rail: Option<String>,
    pub proof: Option<String>,
    pub action_hint: Option<String>,
    pub challenge_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PolicyDecision {
    Free,
    Deny { code: String, message: String },
    Pay(PaymentRequirement),
}

#[derive(Debug, Clone)]
pub struct PaymentRequirement {
    pub action: String,
    pub price: String,
    pub unit: String,
    pub allowed_rails: Vec<String>,
    pub challenge_id: Option<String>,
    pub policy_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SettlementReceipt {
    pub rail: String,
    pub settled_amount: String,
    pub payment_unit: String,
    pub proof_ref: String,
    pub proof_kind: String,
    pub challenge_id: Option<String>,
    pub txn_id: String,
}
