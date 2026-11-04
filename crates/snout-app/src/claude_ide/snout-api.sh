#!/bin/sh
# SnoutCode's editor API, for the agent in its console (the snout-editor skill).
#   snout-api <tool> [JSON arguments]   call a tool: POST /api/<tool>
#   snout-api manual                    the UTCP manual that describes the tools
# The port and the file holding the token's header come from the console's environment.
port=${SNOUT_HOOK_PORT-}
header=${SNOUT_API_HEADER-}
if [ -z "$port" ] || [ -z "$header" ]; then
    echo "snout-api: not in SnoutCode's console, so there is no editor to call" >&2
    exit 2
fi
case ${1-} in
    "" | -h | --help)
        echo "usage: snout-api <tool> [JSON arguments] | snout-api manual" >&2
        exit 2
        ;;
    manual)
        exec curl -sS --fail-with-body -H @"$header" "http://127.0.0.1:$port/utcp"
        ;;
esac
args=${2-}
[ -n "$args" ] || args='{}'
curl -sS --fail-with-body -X POST -H @"$header" -H 'content-type: application/json' \
    --data-binary "$args" "http://127.0.0.1:$port/api/$1"
status=$?
echo
exit $status
