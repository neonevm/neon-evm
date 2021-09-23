#!/bin/bash
set -euo pipefail

SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"

print_non_err_info()
{
  egrep -vn "(use|enum|Err\!|E\!|\/\/\/|==|\_\ =>|EXCLUDE\ Err\!)" $SCRIPTPATH/program/src/*.rs | grep "ProgramError::"
}
export NON_ERR_INFO=$(print_non_err_info | wc -l)
if (("NON_ERR_INFO" > 0)); then
  print_non_err_info>&2
  echo "Please, use macros Err! and E! to add error info!">&2
  exit "$NON_ERR_INFO"
fi

echo "CI checks success"
exit 0
