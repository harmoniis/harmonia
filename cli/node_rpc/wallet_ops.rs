//! Wallet and vault RPC operations.

use harmonia_node_rpc::NodeRpcResult;

use super::helpers::bind_vault_env;

pub(crate) fn wallet_status() -> Result<NodeRpcResult, String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    let wallet_db = crate::paths::wallet_db_path().map_err(|e| e.to_string())?;
    let vault_db = crate::paths::vault_db_path().map_err(|e| e.to_string())?;
    let symbols = harmonia_vault::list_secret_symbols();
    Ok(NodeRpcResult::WalletStatus {
        wallet_db: wallet_db.display().to_string(),
        wallet_present: wallet_db.exists(),
        vault_db: vault_db.display().to_string(),
        vault_present: vault_db.exists(),
        symbol_count: symbols.len(),
    })
}

pub(crate) fn wallet_list_symbols() -> Result<NodeRpcResult, String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    Ok(NodeRpcResult::WalletListSymbols {
        symbols: harmonia_vault::list_secret_symbols(),
    })
}

pub(crate) fn wallet_has_symbol(symbol: &str) -> Result<NodeRpcResult, String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    Ok(NodeRpcResult::WalletHasSymbol {
        symbol: symbol.to_string(),
        present: harmonia_vault::has_secret_for_symbol(symbol),
    })
}

pub(crate) fn wallet_set_secret(symbol: &str, value: &str) -> Result<NodeRpcResult, String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    harmonia_vault::set_secret_for_symbol(symbol, value)?;
    Ok(NodeRpcResult::WalletSetSecret {
        symbol: symbol.to_string(),
    })
}
