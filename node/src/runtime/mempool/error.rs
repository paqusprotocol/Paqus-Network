use paqus::error::CodecError;
use paqus::ledger::LedgerError;
use paqus::transaction::TransactionError;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MempoolError {
    DuplicateTransaction,
    FeeTooLow,
    MempoolFull,
    DuplicateBlockMinerFeeOutput,
    InvalidTransaction(TransactionError),
    InvalidLedgerState(LedgerError),
    CashCoinReserved,
    Serialization(CodecError),
    FeeOverflow,
}

impl fmt::Display for MempoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MempoolError::DuplicateTransaction => {
                f.write_str("transaction already exists in mempool")
            }
            MempoolError::FeeTooLow => f.write_str("transaction fee is below node policy"),
            MempoolError::MempoolFull => f.write_str("mempool transaction limit reached"),
            MempoolError::DuplicateBlockMinerFeeOutput => {
                f.write_str("transaction has more than one block_miner fee output")
            }
            MempoolError::InvalidTransaction(error) => write!(f, "invalid transaction: {error}"),
            MempoolError::InvalidLedgerState(error) => {
                write!(f, "transaction does not fit ledger state: {error}")
            }
            MempoolError::CashCoinReserved => {
                f.write_str("cash coin is already reserved by another mempool transaction")
            }
            MempoolError::Serialization(error) => {
                write!(f, "failed to serialize transaction: {error}")
            }
            MempoolError::FeeOverflow => f.write_str("block transaction fees overflow"),
        }
    }
}

impl Error for MempoolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MempoolError::DuplicateTransaction => None,
            MempoolError::FeeTooLow => None,
            MempoolError::MempoolFull => None,
            MempoolError::DuplicateBlockMinerFeeOutput => None,
            MempoolError::InvalidTransaction(error) => Some(error),
            MempoolError::InvalidLedgerState(error) => Some(error),
            MempoolError::CashCoinReserved => None,
            MempoolError::Serialization(error) => Some(error),
            MempoolError::FeeOverflow => None,
        }
    }
}

impl From<CodecError> for MempoolError {
    fn from(error: CodecError) -> Self {
        Self::Serialization(error)
    }
}

impl From<TransactionError> for MempoolError {
    fn from(error: TransactionError) -> Self {
        MempoolError::InvalidTransaction(error)
    }
}

impl From<LedgerError> for MempoolError {
    fn from(error: LedgerError) -> Self {
        MempoolError::InvalidLedgerState(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_error_retains_codec_cause() {
        let error = MempoolError::from(CodecError::EncodeFailed);
        assert_eq!(
            error.source().unwrap().to_string(),
            "canonical value could not be encoded"
        );
    }
}
