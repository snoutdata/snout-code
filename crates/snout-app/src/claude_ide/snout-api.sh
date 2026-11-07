#!/bin/sh
# SnoutCode's API, for the agent in its console (the snoutcode skill).
#   snout-api <tool> [JSON arguments]   call a tool: POST /api/<tool>
#   snout-api setCommitMessage          the message as plain text on standard input
#   snout-api manual [domain]           the tools, and the arguments of a domain's
# The port and the file holding the token's header come from the console's environment.
# An agent's sandbox may cut it off from the port but not from the session's folder, which holds
# the header. A draft's two calls need nothing else, so there they go through that folder.
port=${SNOUT_HOOK_PORT-}
header=${SNOUT_API_HEADER-}
if [ -z "$port" ] || [ -z "$header" ]; then
    echo "snout-api: not in SnoutCode's console, so there is no editor to call" >&2
    exit 2
fi
case ${1-} in
    "" | -h | --help)
        echo "usage: snout-api <tool> [JSON arguments] | snout-api manual [domain]" >&2
        exit 2
        ;;
    manual)
        exec curl -sS --fail-with-body -G -H @"$header" ${2:+--data-urlencode "domain=$2"} "http://127.0.0.1:$port/manual"
        ;;
esac
# Reached, or turned away (22, with --fail-with-body)? Anything else: the port is out of reach.
unreached() {
    [ "$1" -ne 0 ] && [ "$1" -ne 22 ]
}
dir=${header%[/\\]*}
case $1 in
    getCommitChange)
        answer=$(curl -sS --fail-with-body -X POST -H @"$header" -H 'content-type: application/json' \
            --data-binary '{}' "http://127.0.0.1:$port/api/$1" 2>/dev/null)
        status=$?
        if unreached "$status" && [ -f "$dir/commit-change" ]; then
            cat "$dir/commit-change"
            exit 0
        fi
        printf '%s\n' "$answer"
        exit $status
        ;;
    setCommitMessage)
        if [ $# -eq 1 ]; then
            kept=$dir/commit-message.$$
            cat >"$kept" || exit 1
            if [ ! -s "$kept" ]; then
                rm -f "$kept"
                echo "snout-api: the message is empty" >&2
                exit 2
            fi
            answer=$(curl -sS --fail-with-body -G -X POST -H @"$header" --data-urlencode "message@$kept" \
                "http://127.0.0.1:$port/api/$1" 2>/dev/null)
            status=$?
            if unreached "$status"; then
                mv "$kept" "$dir/commit-message" || exit 1
                echo "The message is in the commit box"
                exit 0
            fi
            rm -f "$kept"
            printf '%s\n' "$answer"
            exit $status
        fi
        ;;
esac
args=${2-}
[ -n "$args" ] || args='{}'
curl -sS --fail-with-body -X POST -H @"$header" -H 'content-type: application/json' \
    --data-binary "$args" "http://127.0.0.1:$port/api/$1"
status=$?
echo
exit $status
