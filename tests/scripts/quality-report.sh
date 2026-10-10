#!/bin/zsh
# quality-report.sh — ddc 质量指标复现（E1 编译错误 + S1 输出行数 + P1 计时）。
# 用法: tests/scripts/quality-report.sh <apk>...
# 依赖: JAVA_HOME (openjdk 21), android.jar + optional 库（与 battery 相同
# classpath；tests/scripts/fetch-classpath.sh 构建）。
export JAVA_HOME=/opt/homebrew/opt/openjdk@21
ROOT=${0:A:h}/../..
DDC=${DDC:-$ROOT/target/release/ddc}
AJ=${AJ:-$HOME/Library/Android/sdk/platforms/android-37.0/android.jar}
LIBJARS=$(ls $ROOT/tests/data/classpath/*.jar 2>/dev/null | tr '\n' ':' | sed 's/:$//')
OPTJARS=$(ls $HOME/Library/Android/sdk/platforms/android-37.0/optional/*.jar 2>/dev/null | tr '\n' ':')
CP="$AJ:$OPTJARS.${LIBJARS:+:$LIBJARS}"
OUT=/tmp/quality-report
mkdir -p $OUT

for apk in "$@"; do
  name=$(basename "$apk" .apk | tr -c 'a-zA-Z0-9_-' '_' | cut -c1-12)
  dir=$OUT/$name
  rm -rf $dir
  # E1 + S1: decompile, gate, count
  s=$(python3 -c 'import time;print(time.time())')
  $DDC "$apk" -o $dir > $OUT/$name.decomp.log 2>&1 || { echo "$name: DECOMP-FAIL"; continue; }
  e=$(python3 -c 'import time;print(time.time())')
  dt=$(python3 -c "print(f'{$e-$s:.1f}')")
  nf=$(find $dir -name '*.java' | wc -l | tr -d ' ')
  nl=$(find $dir -name '*.java' -exec cat {} + | wc -l | tr -d ' ')
  (cd $dir && find . -name '*.java' > $OUT/$name.list \
    && timeout 2400 $JAVA_HOME/bin/javac -Xmaxerrs 500000 -cp "$CP" \
       -d $OUT/$name.cls -nowarn -encoding UTF-8 @$OUT/$name.list \
       > $OUT/$name.javac.log 2>&1)
  tot=$(grep -c '错误:\|error:' $OUT/$name.javac.log)
  env=$(grep -c '程序包.*不存在\|package .* does not exist' $OUT/$name.javac.log)
  rm -rf $OUT/$name.cls
  echo "$name: time=${dt}s files=$nf lines=$nl errors=$tot (env≈$env, ddc≈$((tot-env)))"
done
echo "done: $OUT — 指标定义与基线对照见 docs/quality.md"
