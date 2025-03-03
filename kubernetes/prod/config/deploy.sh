#!/bin/sh
cd /run/secrets/kubernetes.io/serviceaccount
NAMESPACE=$(cat namespace)
DEPLOYMENTS="https://kubernetes.default.svc/apis/apps/v1/namespaces/$NAMESPACE/deployments"
TOKEN=$(cat token)
NOW=$(date -Is)
curl --location --request PATCH "$DEPLOYMENTS/${DEPLOYMENT_NAME}?fieldManager=kubectl-rollout" \
--cacert ca.crt \
--header 'Content-Type: application/strategic-merge-patch+json' \
--header "Authorization: Bearer $TOKEN" \
--data-raw "{\"spec\": {\"template\": {\"metadata\": {\"annotations\": {\"kubectl.kubernetes.io/restartedAt\": \"$NOW\"}}}}}"
