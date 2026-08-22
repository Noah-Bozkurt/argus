#!/usr/bin/env bash
set -euo pipefail

bash -n scripts/first-server-acceptance.sh
bash -n scripts/first-server-product-acceptance.sh
bash -n scripts/first-server-restore-acceptance.sh
bash -n scripts/first-server-reset-reinstall-acceptance.sh
bash -n scripts/reset-first-test.sh
bash -n scripts/test-first-server-acceptance.sh
bash -n scripts/test-first-server-restore-acceptance.sh
bash -n scripts/test-first-server-reset-reinstall-acceptance.sh
bash -n scripts/test-reset-first-test.sh
node --check scripts/first-server-content-acceptance.mjs
bash scripts/test-first-server-acceptance.sh
bash scripts/test-first-server-restore-acceptance.sh
bash scripts/test-first-server-reset-reinstall-acceptance.sh
bash scripts/test-reset-first-test.sh
node --test scripts/first-server-content-acceptance.test.mjs
