#!/usr/bin/env bash
#
# Smoke test executed *inside* a distro container by run.sh.
#
# The harness mounts the pathlint binary at /usr/local/bin/pathlint.
# This script exercises the four CLI surfaces that have a real
# distro dependency — the PATH layout differs between ubuntu, arch,
# and fedora, and we want to know that pathlint copes with each
# without crashing.
#
# It does NOT pin exact human output (that would drift every time a
# detector changes its wording); it pins:
#   - exit codes
#   - subcommand availability
#   - presence of structured fields in --json output
#
# Exit 0 = pass, anything else = fail (set -e propagates the first
# failure).

set -euo pipefail

# ----------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------
say() { printf '   %s\n' "$*"; }
section() { printf '\n[%s]\n' "$*"; }

# ----------------------------------------------------------------
# Identify the distro for the report header
# ----------------------------------------------------------------
distro_name="unknown"
if [[ -r /etc/os-release ]]; then
    # shellcheck disable=SC1091
    source /etc/os-release
    distro_name="${ID:-unknown}"
fi
echo "smoke: distro=${distro_name}"

# ----------------------------------------------------------------
# 1. --version
# ----------------------------------------------------------------
section "--version"
version_output="$(pathlint --version)"
say "${version_output}"
if [[ "${version_output}" != *pathlint* ]]; then
    echo "smoke: --version output missing 'pathlint' token: ${version_output}" >&2
    exit 1
fi

# ----------------------------------------------------------------
# 2. --help (top-level + subcommand)
# ----------------------------------------------------------------
section "--help"
pathlint --help >/dev/null
pathlint check --help >/dev/null
pathlint doctor --help >/dev/null
pathlint trace --help >/dev/null
pathlint sort --help >/dev/null
pathlint catalog list --help >/dev/null
pathlint init --help >/dev/null
say "all subcommands respond to --help"

# ----------------------------------------------------------------
# 3. catalog list (host-independent: ships built-in catalog)
# ----------------------------------------------------------------
section "catalog list"
catalog_lines="$(pathlint catalog list | wc -l)"
say "catalog has ${catalog_lines} lines of output"
if [[ "${catalog_lines}" -lt 10 ]]; then
    echo "smoke: catalog list looks too short: ${catalog_lines} lines" >&2
    exit 1
fi

# ----------------------------------------------------------------
# 4. doctor (uses the container's actual PATH; should never crash)
# ----------------------------------------------------------------
section "doctor (human)"
# Capture and echo for debugging; exit code 0 (clean) or 0 (warns
# but no errors) is acceptable here. doctor only escalates to
# exit 1 if there is a Severity::Error diagnostic — distro defaults
# generally do not produce errors, only warnings.
set +e
doctor_out="$(pathlint doctor 2>&1)"
doctor_code=$?
set -e
say "exit ${doctor_code}"
if [[ ${doctor_code} -ne 0 && ${doctor_code} -ne 1 ]]; then
    echo "smoke: doctor exited with unexpected code ${doctor_code}" >&2
    echo "${doctor_out}" >&2
    exit 1
fi

section "doctor --json"
set +e
doctor_json="$(pathlint doctor --json 2>&1)"
doctor_json_code=$?
set -e
say "exit ${doctor_json_code}"
if [[ ${doctor_json_code} -ne 0 && ${doctor_json_code} -ne 1 ]]; then
    echo "smoke: doctor --json exited with unexpected code ${doctor_json_code}" >&2
    echo "${doctor_json}" >&2
    exit 1
fi
# json must at least be a JSON array (starts with `[`).
if [[ "${doctor_json:0:1}" != "[" ]]; then
    echo "smoke: doctor --json did not produce a JSON array (first char: ${doctor_json:0:1})" >&2
    echo "${doctor_json}" >&2
    exit 1
fi

# ----------------------------------------------------------------
# 5. trace (resolves a guaranteed-present command on each distro)
# ----------------------------------------------------------------
section "trace ls"
# /bin/ls or /usr/bin/ls is on every distro we test.
trace_out="$(pathlint trace ls)"
say "$(printf '%s' "${trace_out}" | head -n 3)"
if [[ "${trace_out}" != *ls* ]]; then
    echo "smoke: trace ls output missing 'ls' token" >&2
    echo "${trace_out}" >&2
    exit 1
fi

section "trace --json ls"
trace_json="$(pathlint trace --json ls)"
if ! grep -q '"kind"' <<<"${trace_json}"; then
    echo "smoke: trace --json missing 'kind' discriminator" >&2
    echo "${trace_json}" >&2
    exit 1
fi
say "kind discriminator present"

# ----------------------------------------------------------------
# 6. check (no rules file; should print no expectations and exit 0)
# ----------------------------------------------------------------
section "check (no rules)"
set +e
check_out="$(pathlint check 2>&1)"
check_code=$?
set -e
say "exit ${check_code}"
if [[ ${check_code} -ne 0 ]]; then
    echo "smoke: check with no rules should exit 0; got ${check_code}" >&2
    echo "${check_out}" >&2
    exit 1
fi

section "check --json (no rules)"
check_json="$(pathlint check --json 2>&1)"
if [[ "${check_json:0:1}" != "[" ]]; then
    echo "smoke: check --json did not produce a JSON array" >&2
    echo "${check_json}" >&2
    exit 1
fi

# ----------------------------------------------------------------
# 7. init (write a starter rules file to a tempdir)
# ----------------------------------------------------------------
section "init"
tmpdir="$(mktemp -d)"
cd "${tmpdir}"
pathlint init >/dev/null
if [[ ! -f pathlint.toml ]]; then
    echo "smoke: pathlint init did not create pathlint.toml" >&2
    exit 1
fi
say "wrote pathlint.toml ($(wc -l <pathlint.toml) lines)"

# ----------------------------------------------------------------
# Done
# ----------------------------------------------------------------
echo
echo "smoke: ${distro_name} OK"
