# Authenticated Wallet Summary

## 1. Scope / Trigger

Applies to `/api/auth/me`, its `/api/users/me` alias, and the shared wallet
summary. The personal UI does not remove wallet settlement or stored balances.

## 2. Signatures

- `AppState::find_auth_user_wallet(&self, user_id: &str) -> Result<Option<StoredWalletSnapshot>, GatewayError>`
- `build_auth_wallet_summary_payload(Option<&StoredWalletSnapshot>) -> serde_json::Value`
- Storage lookup: `WalletLookupKey::UserId` through `GatewayDataState::find_wallet`.

Sources: `apps/aether-gateway/src/handlers/public/support/auth_session.rs`,
`state/runtime/auth/user_provisioning.rs`, and SQLite `src/wallet.rs`.

## 3. Contracts

Resolve and validate the access token, active user and session/device binding
before looking up the wallet using the authenticated user ID. Reuse
`aether_admin::system::serialize_admin_system_users_export_wallet` for present
wallets so auth and administrative exports share the same amounts:

```text
billing.balance = wallet.balance + wallet.gift_balance
billing.recharge_balance = billing.refundable_balance = wallet.balance
billing.unlimited = wallet.limit_mode.eq_ignore_ascii_case("unlimited")
```

Preserve wallet ID, currency, status, limit mode, cumulative amounts and updated
timestamp. A missing wallet keeps the summary object with null ID/timestamp,
zero amounts, `limit_mode: "finite"` and `unlimited: false`.

## 4. Validation & Error Matrix

| Condition | Behavior |
|---|---|
| Missing/expired credentials or invalid session | Existing authentication error |
| Inactive/deleted user | Existing forbidden response |
| Wallet query succeeds with row | Real stored summary |
| Wallet query succeeds without row | Empty finite summary |
| Wallet query fails | HTTP 500 through `build_auth_error_response` |

Do not turn a database error into an empty wallet or an unlimited response.

## 5. Good / Base / Bad Cases

- Good: recharge 12.5 plus gift 3.0 reports balance 15.5 and refundable 12.5.
- Base: no wallet reports zero amounts and does not grant unlimited status.
- Bad: ignore the supplied wallet and return a fixed unlimited object.

## 6. Tests Required

Extend existing auth regressions in `src/tests/frontdoor/public_support.rs`.
Assert authenticated wallet amounts and finite/unlimited/missing behavior.
When changing lookup/error handling, cover repository failure without bypassing
authentication. The adapter's existing wallet tests cover bound user lookup.

## 7. Wrong vs Correct

Wrong: `let wallet = None` after authentication because wallet navigation was
removed. Correct: read the authenticated user's snapshot, propagate failure,
and serialize it with the existing shared helper.
