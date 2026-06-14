#!/usr/bin/env bash
#
# Smoke test executed *inside* a distro container by run.sh.
#
# The harness mounts the pathlint binary at /usr/local/bin/pathlint.
# This script exercises the CLI surfaces that have a real distro
# dependency — the PATH layout differs between ubuntu, arch, and
# fedora, and we want to know that pathlint copes with each without
# crashing.
#
# It does NOT pin exact human output (that would drift every time a
# detector changes its wording); it pins:
#   - exit codes
#   - subcommand availability
#   - presence of structured fields in --json output
#
# Since 0.0.34 (ADR-0028) `doctor` is selfcheck only (binary
# self-locate / config parse / env_lookup) and `lint` carries the
# PATH detectors. The script exercises both: `doctor --json` should
# be a (usually empty) JSON array confirming pathlint runs cleanly
# in this environment; `lint --json` exercises the detectors against
# the container's actual PATH.
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
pathlint lint --help >/dev/null
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
# 4. doctor (selfcheck since 0.0.34: binary self-locate / config
#    parse / env_lookup. Empty diagnostic array means pathlint is
#    healthy in this environment — the canonical container PATH
#    has no config file and no env_lookup failure, so we expect
#    `[]` plus exit 0. PATH analysis lives under `lint` (§5).)
# ----------------------------------------------------------------
section "doctor (human)"
# Capture and echo for debugging; exit code 0 (clean selfcheck) is
# the expected case in a vanilla distro container. Severity::Error
# would escalate to exit 1; we accept that too for robustness.
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
# 5. lint (PATH analysis since 0.0.34 — the detectors that used to
#    live under `doctor` moved here. The container's PATH typically
#    surfaces shortenable / writable / duplicate findings; we only
#    pin that the output is a JSON array and the exit code is in
#    {0, 1} — exit 0 if clean, 1 if there is any diagnostic.)
# ----------------------------------------------------------------
section "lint --json"
set +e
lint_json="$(pathlint lint --json 2>&1)"
lint_json_code=$?
set -e
say "exit ${lint_json_code}"
if [[ ${lint_json_code} -ne 0 && ${lint_json_code} -ne 1 ]]; then
    echo "smoke: lint --json exited with unexpected code ${lint_json_code}" >&2
    echo "${lint_json}" >&2
    exit 1
fi
if [[ "${lint_json:0:1}" != "[" ]]; then
    echo "smoke: lint --json did not produce a JSON array (first char: ${lint_json:0:1})" >&2
    echo "${lint_json}" >&2
    exit 1
fi

# ----------------------------------------------------------------
# 6. trace (resolves a guaranteed-present command on each distro)
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
# 7. check (no rules file; should print no expectations and exit 0)
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
# 8. init (write a starter rules file to a tempdir)
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
