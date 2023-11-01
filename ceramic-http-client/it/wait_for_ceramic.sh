#!/usr/bin/env bash

while [ $(curl -s -o /dev/null -I -w "%{http_code}" "http://localhost:7007/api/v0/node/healthcheck") -ne "200" ]; do
  echo "Ceramic is not yet ready, waiting and trying again"
  sleep 1
done