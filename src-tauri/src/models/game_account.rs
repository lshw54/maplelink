//! Game account and credential models.

use serde::{Deserialize, Serialize};

/// A Beanfun sub-account associated with a specific game.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAccount {
    pub id: String,
    pub display_name: String,
    pub game_type: String,
    pub sn: String,
    pub status: String,
    pub created_at: String,
}

/// One-time game credentials retrieved from Beanfun for launching.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCredentials {
    pub account_id: String,
    /// One-time password (10-digit).
    pub otp: String,
    pub retrieved_at: chrono::DateTime<chrono::Utc>,
    /// Command line template from service INI (e.g. "server port BeanFun %s %s").
    /// `%s` placeholders are replaced with account_id and otp at launch time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_line_template: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy for generating a random [`GameAccount`] with non-empty fields.
    fn arb_game_account() -> impl Strategy<Value = GameAccount> {
        (
            "[a-zA-Z0-9]{4,16}",
            "[a-zA-Z0-9 ]{3,20}",
            "[a-zA-Z0-9]{2,10}",
            "[0-9]{6,12}",
            "active|inactive",
            "2020-01-01",
        )
            .prop_map(|(id, display_name, game_type, sn, status, created_at)| {
                GameAccount {
                    id,
                    display_name,
                    game_type,
                    sn,
                    status,
                    created_at,
                }
            })
    }

    // Feature: maplelink-rewrite, Property 5: Game accounts contain required display fields
    //
    // For any list of GameAccount objects returned by the Beanfun service,
    // every account shall have a non-empty display_name and a non-empty game_type.
    //
    // **Validates: Requirements 2.1**
    proptest! {
        #[test]
        fn prop_game_accounts_have_required_display_fields(
            accounts in proptest::collection::vec(arb_game_account(), 0..20),
        ) {
            for account in &accounts {
                prop_assert!(
                    !account.display_name.is_empty(),
                    "GameAccount display_name must not be empty, got id={}",
                    account.id
                );
                prop_assert!(
                    !account.game_type.is_empty(),
                    "GameAccount game_type must not be empty, got id={}",
                    account.id
                );
            }
        }
    }
}
