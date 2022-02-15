#!/bin/bash
set -euox pipefail

SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"

print_non_err_info()
{
  egrep -vn "(use|enum|Err\!|E\!|\/\/\/|==|\_\ =>|EXCLUDE\ Err\!|ProgramError::Custom\(0\) =>)" $SCRIPTPATH/program/src/*.rs | grep "ProgramError::"
}
export NON_ERR_INFO=$(print_non_err_info | wc -l)
if (("NON_ERR_INFO" > 0)); then
  print_non_err_info>&2
  echo "Please, use macros Err! and E! to add error info!">&2
  exit "$NON_ERR_INFO"
fi


INFRA_REFLECT_FILE="https://github.com/neonlabsorg/neon-infra-inventories/blob/369-calculate-hashes/develop_changes/neon-evm.changes"
MAINTENANCE_FILES="
./program/src/config.rs
./deploy-evm.sh
./deploy-test.sh
./docker-compose-test.yml"

cat ./neon-evm.changes

echo "INFRA_REFLECT_FILE=INFRA_REFLECT_FILE"
echo "MAINTENANCE_FILES=$MAINTENANCE_FILES"

git ls-files -s $MAINTENANCE_FILES > neon-evm.changes.${REVISION}
wget "$INFRA_REFLECT_FILE"

if diff neon-evm.changes neon-evm.changes.${REVISION}; then
  echo "the changes in maintenance files: "$MAINTENANCE_FILES "are reflected in the infra file $INFRA_REFLECT_FILE";
else
  echo "the changes in maintenance files: "$MAINTENANCE_FILES "are NOT reflected in the infra file $INFRA_REFLECT_FILE";
fi

echo "CI checks success"
exit 0
