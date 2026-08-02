# AWS-blocked Bedrock models

Observed on 2026-07-31 using the harness `BEDROCK_API_KEY` in `us-east-1`.
"Blocked" here means unavailable to this AWS account; it does not mean the
model is absent from the regional Bedrock catalog.

## Block confirmed by real inference

- `openai.gpt-5.6-sol` — Mantle Responses API returned HTTP 401
  `access_denied`.
- `openai.gpt-5.6-terra` — Mantle Responses API returned HTTP 401
  `access_denied`.
- `openai.gpt-5.6-luna` — Mantle Responses API returned HTTP 401
  `access_denied`.
- `anthropic.claude-fable-5` — Bedrock Runtime returned
  `AccessDeniedException`.
- `anthropic.claude-sonnet-5` — Bedrock Runtime returned
  `AccessDeniedException`.
- `anthropic.claude-opus-5` — Bedrock Runtime returned
  `AccessDeniedException`.
- `anthropic.claude-opus-4-8` — the correct Runtime US inference profile
  `us.anthropic.claude-opus-4-8` returned `AccessDeniedException`, and the
  Mantle Messages endpoint returned HTTP 403 `permission_error`.

## Block confirmed by account-specific model status

- `anthropic.claude-haiku-4-5` — Mantle `/v1/models/{model}` reports
  `unavailable`.
- `anthropic.claude-opus-4-7` — Mantle reports `unavailable`; Runtime also
  reports agreement status `NOT_AVAILABLE`.

## What was ruled out

- The API key authenticates successfully.
- All listed models appear in the bulk Bedrock Mantle catalog.
- The account retention mode is `provider_data_share` and is allowed by every
  listed model.
- The model IDs and `us-east-1` region are valid.
- Runtime reports authorization, entitlement, and Region availability for the
  checked Anthropic models, despite real invocation failures.

The observed blocker is AWS account-level model availability/provisioning.
Send this complete model list to AWS Support or AWS Sales when requesting that
the account entitlement be corrected.
