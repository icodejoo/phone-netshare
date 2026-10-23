#!/usr/bin/env bash
# 构建 phone-stop APK：javac + d8 出 dex，aapt2 打包，zipalign 对齐，apksigner 签名。
# 需要：ANDROID_HOME、JDK。签名用仓库内固定的 phone-stop/stop.keystore（口令 android），
# 保证每次构建签名一致，手机上才能覆盖安装；APK_OUT 可改输出路径。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
BT="$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)"
JAR="$(ls -d "$SDK"/platforms/android-* | sort -V | tail -1)/android.jar"
KS="${KEYSTORE:-$ROOT/phone-stop/stop.keystore}"
PASS="pass:${KS_PASS:-android}"
APK_OUT="${APK_OUT:-$ROOT/bin/phone-stop.apk}"
OUT="$ROOT/phone-stop/build"

cd "$ROOT/phone-stop"
rm -rf "$OUT" && mkdir -p "$OUT/classes" "$OUT/dex"
javac --release 8 -Xlint:-options -cp "$JAR" -d "$OUT/classes" $(find src -name '*.java')
java -cp "$BT/lib/d8.jar" com.android.tools.r8.D8 --release --min-api 24 --lib "$JAR" \
  --output "$OUT/dex" $(find "$OUT/classes" -name '*.class')

"$BT/aapt2" compile --dir res -o "$OUT/res.zip"
"$BT/aapt2" link -I "$JAR" --manifest AndroidManifest.xml -o "$OUT/base.apk" "$OUT/res.zip"
(cd "$OUT/dex" && "$BT/aapt" add ../base.apk classes.dex)
"$BT/zipalign" -f -p 4 "$OUT/base.apk" "$OUT/aligned.apk"
mkdir -p "$(dirname "$APK_OUT")"
java -jar "$BT/lib/apksigner.jar" sign --ks "$KS" --ks-pass "$PASS" \
  --key-pass "$PASS" --out "$APK_OUT" "$OUT/aligned.apk"
echo "产物：$APK_OUT"
