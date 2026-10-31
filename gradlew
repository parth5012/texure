#!/usr/bin/env bash
set -e
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ -z "$ANDROID_HOME" ] && [ -d "/home/parth/Android/Sdk" ]; then
  export ANDROID_HOME="/home/parth/Android/Sdk"
fi

if [ -z "$JAVA_HOME" ] && [ -d "/home/parth/.local/jdk-17" ]; then
  export JAVA_HOME="/home/parth/.local/jdk-17"
fi

if [ -n "$JAVA_HOME" ] && [ -d "$JAVA_HOME/bin" ]; then
  export PATH="$JAVA_HOME/bin:$PATH"
fi

# Remap :android:module to :module for convenience if called from root
ARGS=()
for arg in "$@"; do
  if [[ "$arg" == :android:* ]]; then
    ARGS+=("${arg#:android}")
  else
    ARGS+=("$arg")
  fi
done

cd "$ROOT_DIR/android" && exec ./gradlew "${ARGS[@]}"
