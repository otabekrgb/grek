#!/usr/bin/env bash
# Собирает папку hosting/ из исходников проекта и публикует её на GitHub.
#   GH_USER=логин GH_TOKEN=токен bash hosting/publish.sh
#   GH_REPO=grek — имя репозитория (по умолчанию grek)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
: "${GH_USER:?Укажите GH_USER — логин на github.com}"
: "${GH_TOKEN:?Укажите GH_TOKEN — токен с правом repo}"
repo="${GH_REPO:-grek}"

mkdir -p "$here/src" "$here/assets" "$here/voice/lines"
cp "$root/Cargo.toml" "$root/Cargo.lock" "$here/"
cp "$root"/src/*.rs "$root"/src/*.html "$here/src/"
cp "$root"/assets/*.jpg "$here/assets/"
cp "$root"/voice/lines/*.wav "$here/voice/lines/"

api="https://api.github.com"
exists=$(curl -s -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer ${GH_TOKEN}" "$api/repos/${GH_USER}/${repo}")
if [ "$exists" = "404" ]; then
  created=$(curl -s -o /dev/null -w '%{http_code}' -X POST \
    -H "Authorization: Bearer ${GH_TOKEN}" -H "Accept: application/vnd.github+json" \
    -d "{\"name\":\"${repo}\",\"private\":false}" "$api/user/repos")
  [ "$created" = "201" ] || { echo "Не удалось создать репозиторий (HTTP $created).";
    echo "Создайте пустой репозиторий ${GH_USER}/${repo} вручную и запустите снова."; exit 1; }
  echo "Создан репозиторий https://github.com/${GH_USER}/${repo}"
fi

cd "$here"
if [ ! -d .git ]; then
  git init -q
  git symbolic-ref HEAD refs/heads/main
fi
git add -A
git -c user.name="grek" -c user.email="grek@localhost" \
  commit -q -m "sync $(date '+%Y-%m-%d %H:%M')" || true
git push --force "https://x-access-token:${GH_TOKEN}@github.com/${GH_USER}/${repo}.git" HEAD:main

echo "Опубликовано: https://github.com/${GH_USER}/${repo}"
