#!/usr/bin/env bash
set -euo pipefail

apply_secrets=false
wallet_dir="Wallet_deductibledb"

for arg in "$@"; do
  case "$arg" in
    --apply)
      apply_secrets=true
      ;;
    *)
      wallet_dir="$arg"
      ;;
  esac
done

if [ ! -d "$wallet_dir" ]; then
  echo "Wallet directory not found: $wallet_dir" >&2
  exit 1
fi

if ! command -v zip >/dev/null 2>&1; then
  echo "zip is required to package the wallet directory" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

wallet_zip="$tmp_dir/wallet.zip"
wallet_b64="$tmp_dir/wallet.b64"

(
  cd "$wallet_dir"
  zip -qr "$wallet_zip" .
)

if base64 --help 2>&1 | grep -q -- '-w'; then
  base64 -w 0 "$wallet_zip" > "$wallet_b64"
else
  base64 -i "$wallet_zip" | tr -d '\n' > "$wallet_b64"
fi

if command -v sha256sum >/dev/null 2>&1; then
  wallet_sha="$(sha256sum "$wallet_zip" | awk '{print $1}')"
else
  wallet_sha="$(shasum -a 256 "$wallet_zip" | awk '{print $1}')"
fi

echo "Wallet zip: $wallet_zip"
echo "Wallet zip bytes: $(wc -c < "$wallet_zip")"
echo "Wallet base64 chars: $(wc -c < "$wallet_b64")"
echo "Wallet SHA256: $wallet_sha"
echo

if [ "$apply_secrets" = true ]; then
  if ! command -v gh >/dev/null 2>&1; then
    echo "gh CLI is required for --apply" >&2
    exit 1
  fi

  gh secret set WALLET_ZIP_BASE64 < "$wallet_b64"
  printf '%s' "$wallet_sha" | gh secret set WALLET_ZIP_SHA256
  echo "Updated GitHub secrets: WALLET_ZIP_BASE64 and WALLET_ZIP_SHA256"
else
  echo "Run this to update repository secrets automatically:"
  echo "  ./scripts/prepare-wallet-secret.sh --apply \"$wallet_dir\""
fi

echo
echo "The zip is created from the wallet directory contents, not the parent folder, so the workflow extracts files into Wallet_deductibledb/ without an extra nested directory."