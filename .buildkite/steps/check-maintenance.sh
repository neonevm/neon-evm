#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

INFRA_REFLECT_FILE="neon-evm.changes"
INFRA_REFLECT_REPO_PATH="https://github.com/neonlabsorg/neon-infra-inventories/blob/develop/develop_changes/"
MAINTENANCE_FILES="
./evm_loader/program/src/config.rs"

echo "MAINTENANCE_FILES=$MAINTENANCE_FILES"
rm -rf ./neon-infra-inventories/
git clone -b develop https://github.com/neonlabsorg/neon-infra-inventories.git
cd ./neon-infra-inventories
git checkout -b ${BUILDKITE_BRANCH} || echo "There is no branch with the same name. develop will be used."
cd ..

git ls-files -s $MAINTENANCE_FILES > "${INFRA_REFLECT_FILE}"".""${REVISION}"
echo "------ ${INFRA_REFLECT_FILE}:" && cat ./neon-infra-inventories/develop_changes/"${INFRA_REFLECT_FILE}"
echo "------ ${INFRA_REFLECT_FILE}.${REVISION}:" && cat ./"${INFRA_REFLECT_FILE}"".""${REVISION}"
echo "==========================================================================="
if diff -B ./neon-infra-inventories/develop_changes/"${INFRA_REFLECT_FILE}" ./"${INFRA_REFLECT_FILE}"".""${REVISION}"; then
  rm -rf ./neon-infra-inventories/
  echo "==========================================================================="
  echo "The changes in maintenance files: "$MAINTENANCE_FILES "are reflected in the infra file ${INFRA_REFLECT_REPO_PATH}${INFRA_REFLECT_FILE}";
else
  rm -rf ./neon-infra-inventories/
  echo "==========================================================================="
  echo "The changes in maintenance files: "$MAINTENANCE_FILES "are NOT reflected in the infra file ${INFRA_REFLECT_REPO_PATH}${INFRA_REFLECT_FILE}" | grep --color=always "are NOT reflected";
  if [[ ${BUILDKITE_BRANCH} == "develop" ]]; then
       exit 1
  fi
fi
echo "==========================================================================="
